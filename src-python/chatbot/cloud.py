"""Copies Nyra conversations to the signed-in ASAO account, so they follow the user to other PCs.

The local store stays the source of truth: Nyra reads and writes it as before, and this
module syncs it with the ASAO accounts API (accounts-api/, on Vercel) in the background.

- Push: new or changed messages, renamed chats and deleted chats are sent after each change.
- Pull: on sign-in, and at most once a minute when the chat list is opened, conversations
  from other PCs are fetched and chats deleted elsewhere are removed here.

ASAO tells Nyra who is signed in through request headers (see `account` in main.py).
Failures (offline, server down) are kept for the status endpoint and retried on the next
change or pull; nothing is lost because unsent changes stay marked in the store.
"""

import asyncio
import json
import logging
import time
from typing import Any, Awaitable, Callable

import httpx

from .history import ConversationStore

log = logging.getLogger("nyra.cloud")

# The accounts API refuses messages over 256 KiB; stay under it.
MAX_CLOUD_PAYLOAD = 200_000
# Vercel limits request bodies to 4.5 MB.
MAX_BATCH_BYTES = 3_000_000
MAX_BATCH_MESSAGES = 400
PULL_INTERVAL_S = 60
TIMEOUT = httpx.Timeout(20.0, connect=10.0)


def cloud_payload(payload: str) -> str:
    """The message as saved online: oversized activity traces are trimmed, never the answer."""
    if len(payload.encode()) <= MAX_CLOUD_PAYLOAD:
        return payload
    try:
        message = json.loads(payload)
    except ValueError:
        return payload[: MAX_CLOUD_PAYLOAD // 4]
    run = message.get("run") or {}
    for node in run.get("activities", []):
        node.pop("args", None)
        node["progress"] = node.get("progress", [])[-1:]
        if isinstance(node.get("summary"), str):
            node["summary"] = node["summary"][:500]
    trimmed = json.dumps(message, ensure_ascii=False, default=str)
    if len(trimmed.encode()) <= MAX_CLOUD_PAYLOAD:
        return trimmed
    message.pop("run", None)
    content = message.get("content") or ""
    while len(json.dumps(message, ensure_ascii=False, default=str).encode()) > MAX_CLOUD_PAYLOAD and content:
        content = content[: len(content) * 3 // 4]
        message["content"] = content + "\n\n(Shortened when saved online.)"
    return json.dumps(message, ensure_ascii=False, default=str)


class CloudError(Exception):
    def __init__(self, status: int, message: str):
        super().__init__(message)
        self.status = status


class CloudSync:
    def __init__(self, store: ConversationStore, on_remote_delete: Callable[[str], Awaitable[None]]):
        self.store = store
        self.on_remote_delete = on_remote_delete
        self.owner: str | None = None
        self.token: str | None = None
        self.url: str | None = None
        self.enabled = False
        self.last_sync_at: int | None = None
        self.last_error: str | None = None
        self._client: httpx.AsyncClient | None = None
        self._lock = asyncio.Lock()
        self._task: asyncio.Task[None] | None = None
        self._pending = False
        self._want_pull = False
        self._last_pull = 0.0

    @property
    def active(self) -> bool:
        return bool(self.enabled and self.owner and self.token and self.url)

    def configure(self, owner: str | None, token: str | None, url: str | None, enabled: bool) -> bool:
        """Updates who is signed in. Returns True when the account or setting changed."""
        if (owner, token, url, enabled) == (self.owner, self.token, self.url, self.enabled):
            return False
        account_changed = (owner, token) != (self.owner, self.token)
        self.owner, self.token, self.url, self.enabled = owner, token, url, enabled
        if account_changed:
            self.last_error = None
            self._last_pull = 0.0
        return True

    def status(self) -> dict[str, Any]:
        return {
            "enabled": self.enabled,
            "signedIn": bool(self.owner and self.token),
            "syncing": self._task is not None and not self._task.done(),
            "lastSyncAt": self.last_sync_at,
            "error": self.last_error,
        }

    # -- Scheduling -----------------------------------------------------------------

    def schedule(self, pull: bool = False) -> None:
        """Runs a sync in the background; requests made while one runs are merged into the next."""
        if not self.active:
            return
        self._pending = True
        self._want_pull = self._want_pull or pull
        if self._task is None or self._task.done():
            self._task = asyncio.create_task(self._run())

    def pull_if_stale(self) -> None:
        if time.monotonic() - self._last_pull > PULL_INTERVAL_S:
            self.schedule(pull=True)

    async def _run(self) -> None:
        while self._pending and self.active:
            self._pending = False
            pull, self._want_pull = self._want_pull, False
            try:
                await self.sync(pull)
            except Exception as exc:  # reported through status(); retried on the next change
                log.warning("chat sync failed: %s", exc)
                self.last_error = str(exc)

    async def sync(self, pull: bool) -> None:
        async with self._lock:
            owner = self.owner
            if not owner or not self.active:
                return
            if pull:
                await self._pull(owner)
                self._last_pull = time.monotonic()
            await self._push(owner)
            self.last_sync_at = int(time.time() * 1000)
            self.last_error = None

    # -- HTTP -----------------------------------------------------------------------

    async def _request(self, method: str, path: str, body: Any = None) -> Any:
        if self._client is None:
            self._client = httpx.AsyncClient(timeout=TIMEOUT)
        try:
            res = await self._client.request(
                method,
                f"{self.url}/api/{path}",
                json=body,
                headers={"Authorization": f"Bearer {self.token}"},
            )
        except httpx.HTTPError as exc:
            raise CloudError(0, "Couldn't reach the account server.") from exc
        if res.status_code == 204:
            return None
        try:
            data = res.json()
        except ValueError:
            data = None
        if res.status_code >= 400:
            message = data.get("error") if isinstance(data, dict) else None
            raise CloudError(res.status_code, message or f"The account server answered {res.status_code}.")
        return data

    async def close(self) -> None:
        if self._task is not None:
            self._task.cancel()
        if self._client is not None:
            await self._client.aclose()
            self._client = None

    # -- Push and pull ----------------------------------------------------------------

    async def _push(self, owner: str) -> None:
        for cid in await self.store.deletions(owner):
            await self._request("POST", "chat-delete", {"id": cid})
            await self.store.forget_deletion(cid)

        for item in await self.store.unsent(owner):
            chat, messages = item["chat"], item["messages"]
            batches: list[list[dict[str, Any]]] = [[]]
            size = 0
            for m in messages:
                payload = cloud_payload(m["payload"])
                n = len(payload.encode())
                if batches[-1] and (size + n > MAX_BATCH_BYTES or len(batches[-1]) >= MAX_BATCH_MESSAGES):
                    batches.append([])
                    size = 0
                batches[-1].append({**m, "payload": payload, "_local": m["payload"]})
                size += n
            for i, batch in enumerate(batches):
                body = {"chat": chat, "messages": [{k: v for k, v in m.items() if k != "_local"} for m in batch]}
                try:
                    await self._request("POST", "chat-sync", body)
                except CloudError as exc:
                    if exc.status == 410:  # deleted on another PC
                        await self.store.delete(chat["id"], owner, remember=False)
                        await self.on_remote_delete(chat["id"])
                        break
                    raise
                await self.store.mark_sent(
                    chat["id"], {m["id"]: m["_local"] for m in batch}, chat_done=i == len(batches) - 1
                )

    async def _pull(self, owner: str) -> None:
        remote = await self._request("GET", "chats")
        local = await self.store.sync_state(owner)
        pending_deletes = set(await self.store.deletions(owner))
        for chat in remote or []:
            cid = chat.get("id")
            if not isinstance(cid, str) or cid in pending_deletes:
                continue
            mine = local.get(cid)
            if chat.get("deleted"):
                if mine is not None:
                    await self.store.delete(cid, owner, remember=False)
                    await self.on_remote_delete(cid)
                continue
            if mine is None or chat["updatedAt"] > mine["updatedAt"]:
                data = await self._request("GET", f"chat?id={cid}")
                await self.store.apply_remote(owner, data["chat"], data["messages"])
            elif chat.get("title") and chat["title"] != mine["title"] and not mine["dirty"]:
                await self.store.set_title_from_remote(cid, chat["title"])

    async def delete_everything_online(self) -> None:
        """Removes the account's online copy. Chats on this PC stay and count as not sent."""
        if not (self.owner and self.token and self.url):
            raise CloudError(401, "Sign in to ASAO first.")
        if self.enabled:
            raise CloudError(409, "Turn off chat syncing first, or this PC uploads its chats again.")
        async with self._lock:
            await self._request("POST", "chats-delete-all")
            await self.store.mark_all_unsent(self.owner)
