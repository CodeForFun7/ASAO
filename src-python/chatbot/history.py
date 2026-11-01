"""Saved conversations.

Two layers live in the same SQLite file (`nyra.db` in ASAO's app data folder):
- LangGraph's AsyncSqliteSaver keeps each conversation's full model state (messages, tool
  calls and results) keyed by thread id = conversation id. That is what gives follow-up
  questions their context.
- This store keeps what the chat shows: titles, and each message with its live activity
  trace (tools, agents, confirmations, plan), so past conversations can be reopened.

Each conversation belongs to the ASAO account (email) that was signed in when it started,
and only that account sees it. `dirty`, `synced` and `nyra_deleted` track what still has to
be sent to the account's online copy (see cloud.py).
"""

# Lazy annotations: inside ConversationStore, `list` is also a method name.
from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any

import aiosqlite

SCHEMA = """
CREATE TABLE IF NOT EXISTS nyra_conversations (
    id          TEXT    PRIMARY KEY,
    title       TEXT    NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_nyra_conversations_updated ON nyra_conversations (updated_at DESC);
CREATE TABLE IF NOT EXISTS nyra_messages (
    id               TEXT    PRIMARY KEY,
    conversation_id  TEXT    NOT NULL,
    seq              INTEGER NOT NULL,
    role             TEXT    NOT NULL,
    created_at       INTEGER NOT NULL,
    payload          TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_nyra_messages_seq ON nyra_messages (conversation_id, seq);
CREATE TABLE IF NOT EXISTS nyra_deleted (
    id     TEXT PRIMARY KEY,
    owner  TEXT NOT NULL
);
"""

# Columns added after the first release, created on open when missing.
MIGRATIONS = [
    ("nyra_conversations", "owner", "ALTER TABLE nyra_conversations ADD COLUMN owner TEXT"),
    ("nyra_conversations", "dirty", "ALTER TABLE nyra_conversations ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1"),
    ("nyra_messages", "synced", "ALTER TABLE nyra_messages ADD COLUMN synced INTEGER NOT NULL DEFAULT 0"),
]

MAX_PAYLOAD_CHARS = 3_000_000


def _now() -> int:
    return int(time.time() * 1000)


class ConversationStore:
    def __init__(self, path: Path):
        self.path = path
        self.db: aiosqlite.Connection | None = None

    async def open(self) -> None:
        self.db = await aiosqlite.connect(self.path)
        await self.db.execute("PRAGMA journal_mode = WAL")
        await self.db.executescript(SCHEMA)
        for table, column, statement in MIGRATIONS:
            cur = await self.db.execute(f"PRAGMA table_info({table})")
            if column not in {row[1] for row in await cur.fetchall()}:
                await self.db.execute(statement)
        await self.db.execute(
            "CREATE INDEX IF NOT EXISTS idx_nyra_conversations_owner ON nyra_conversations (owner, updated_at DESC)"
        )
        await self.db.commit()

    async def close(self) -> None:
        if self.db is not None:
            await self.db.close()
            self.db = None

    def _conn(self) -> aiosqlite.Connection:
        if self.db is None:
            raise RuntimeError("Conversation store is not open")
        return self.db

    async def claim_unowned(self, owner: str) -> None:
        """Conversations from before accounts existed go to the first account signed in here."""
        await self._conn().execute("UPDATE nyra_conversations SET owner = ? WHERE owner IS NULL", (owner,))
        await self._conn().commit()

    async def list(self, owner: str | None, limit: int = 100) -> list[dict[str, Any]]:
        cur = await self._conn().execute(
            """SELECT c.id, c.title, c.created_at, c.updated_at,
                      (SELECT COUNT(*) FROM nyra_messages m WHERE m.conversation_id = c.id)
               FROM nyra_conversations c WHERE c.owner IS ? ORDER BY c.updated_at DESC LIMIT ?""",
            (owner, max(1, min(limit, 500))),
        )
        rows = await cur.fetchall()
        return [
            {"id": r[0], "title": r[1], "createdAt": r[2], "updatedAt": r[3], "messageCount": r[4]}
            for r in rows
        ]

    async def owns(self, conversation_id: str, owner: str | None) -> bool | None:
        """True if the account owns the conversation, False if another does, None if it doesn't exist."""
        cur = await self._conn().execute("SELECT owner FROM nyra_conversations WHERE id = ?", (conversation_id,))
        row = await cur.fetchone()
        return None if row is None else row[0] == owner

    async def get(self, conversation_id: str, owner: str | None) -> dict[str, Any] | None:
        db = self._conn()
        cur = await db.execute(
            "SELECT id, title, created_at, updated_at FROM nyra_conversations WHERE id = ? AND owner IS ?",
            (conversation_id, owner),
        )
        row = await cur.fetchone()
        if row is None:
            return None
        cur = await db.execute(
            "SELECT payload FROM nyra_messages WHERE conversation_id = ? ORDER BY seq", (conversation_id,)
        )
        messages = []
        for (payload,) in await cur.fetchall():
            try:
                messages.append(json.loads(payload))
            except ValueError:
                continue  # one damaged row must not hide the conversation
        return {"id": row[0], "title": row[1], "createdAt": row[2], "updatedAt": row[3], "messageCount": len(messages), "messages": messages}

    async def transcript(self, conversation_id: str) -> list[tuple[str, str]]:
        """(role, text) of each message, oldest first, for rebuilding the model's context."""
        cur = await self._conn().execute(
            "SELECT role, payload FROM nyra_messages WHERE conversation_id = ? ORDER BY seq", (conversation_id,)
        )
        out = []
        for role, payload in await cur.fetchall():
            try:
                content = json.loads(payload).get("content") or ""
            except ValueError:
                continue
            if content.strip():
                out.append((role, content))
        return out

    async def save_message(
        self, conversation_id: str, title: str, message: dict[str, Any], owner: str | None = None
    ) -> None:
        """Creates the conversation if needed and inserts or replaces the message by id."""
        payload = json.dumps(message, ensure_ascii=False, default=str)
        if len(payload) > MAX_PAYLOAD_CHARS:
            # Keep the answer; drop the bulky trace details.
            run = message.get("run") or {}
            for node in run.get("activities", []):
                node.pop("args", None)
                node["progress"] = node.get("progress", [])[-3:]
            payload = json.dumps(message, ensure_ascii=False, default=str)[:MAX_PAYLOAD_CHARS]
        now = _now()
        db = self._conn()
        await db.execute(
            """INSERT INTO nyra_conversations (id, title, created_at, updated_at, owner, dirty) VALUES (?, ?, ?, ?, ?, 1)
               ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at, dirty = 1""",
            (conversation_id, (title.strip() or "New chat")[:120], now, now, owner),
        )
        await db.execute(
            """INSERT INTO nyra_messages (id, conversation_id, seq, role, created_at, payload, synced)
               VALUES (?, ?, COALESCE((SELECT MAX(seq) + 1 FROM nyra_messages WHERE conversation_id = ?), 0), ?, ?, ?, 0)
               ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, synced = 0""",
            (message["id"], conversation_id, conversation_id, message.get("role", "assistant"), message.get("createdAt", now), payload),
        )
        await db.commit()

    async def rename(self, conversation_id: str, title: str, owner: str | None) -> bool:
        cur = await self._conn().execute(
            "UPDATE nyra_conversations SET title = ?, dirty = 1 WHERE id = ? AND owner IS ?",
            (title.strip()[:120], conversation_id, owner),
        )
        await self._conn().commit()
        return cur.rowcount > 0

    async def delete(self, conversation_id: str, owner: str | None, remember: bool = True) -> None:
        """Deletes a conversation. With `remember`, the deletion is later sent to the account."""
        db = self._conn()
        await db.execute("DELETE FROM nyra_messages WHERE conversation_id = ?", (conversation_id,))
        await db.execute("DELETE FROM nyra_conversations WHERE id = ? AND owner IS ?", (conversation_id, owner))
        if remember and owner:
            await db.execute(
                "INSERT INTO nyra_deleted (id, owner) VALUES (?, ?) ON CONFLICT(id) DO NOTHING", (conversation_id, owner)
            )
        await db.commit()

    # -- Sync with the account's online copy (cloud.py) ----------------------------

    async def sync_state(self, owner: str) -> dict[str, dict[str, Any]]:
        cur = await self._conn().execute(
            "SELECT id, title, updated_at, dirty FROM nyra_conversations WHERE owner = ?", (owner,)
        )
        return {r[0]: {"title": r[1], "updatedAt": r[2], "dirty": bool(r[3])} for r in await cur.fetchall()}

    async def deletions(self, owner: str) -> list[str]:
        cur = await self._conn().execute("SELECT id FROM nyra_deleted WHERE owner = ?", (owner,))
        return [r[0] for r in await cur.fetchall()]

    async def forget_deletion(self, conversation_id: str) -> None:
        await self._conn().execute("DELETE FROM nyra_deleted WHERE id = ?", (conversation_id,))
        await self._conn().commit()

    async def unsent(self, owner: str) -> list[dict[str, Any]]:
        """Conversations with changes the account doesn't have yet, with their unsent messages."""
        db = self._conn()
        cur = await db.execute(
            """SELECT c.id, c.title, c.created_at, c.updated_at FROM nyra_conversations c
               WHERE c.owner = ? AND (c.dirty = 1 OR EXISTS
                 (SELECT 1 FROM nyra_messages m WHERE m.conversation_id = c.id AND m.synced = 0))""",
            (owner,),
        )
        out = []
        for cid, title, created, updated in await cur.fetchall():
            mcur = await db.execute(
                """SELECT id, seq, role, created_at, payload FROM nyra_messages
                   WHERE conversation_id = ? AND synced = 0 ORDER BY seq""",
                (cid,),
            )
            messages = [
                {"id": m[0], "seq": m[1], "role": m[2], "createdAt": m[3], "payload": m[4]}
                for m in await mcur.fetchall()
            ]
            out.append({"chat": {"id": cid, "title": title, "createdAt": created, "updatedAt": updated}, "messages": messages})
        return out

    async def mark_sent(self, conversation_id: str, message_payloads: dict[str, str], chat_done: bool) -> None:
        """Marks messages as sent, unless they changed again while the request was in flight."""
        db = self._conn()
        for mid, payload in message_payloads.items():
            await db.execute("UPDATE nyra_messages SET synced = 1 WHERE id = ? AND payload = ?", (mid, payload))
        if chat_done:
            await db.execute(
                """UPDATE nyra_conversations SET dirty = 0 WHERE id = ? AND NOT EXISTS
                     (SELECT 1 FROM nyra_messages m WHERE m.conversation_id = ? AND m.synced = 0)""",
                (conversation_id, conversation_id),
            )
        await db.commit()

    async def apply_remote(self, owner: str, chat: dict[str, Any], messages: list[dict[str, Any]]) -> None:
        """Takes a conversation from the account. Local changes not yet sent are kept."""
        db = self._conn()
        await db.execute(
            """INSERT INTO nyra_conversations (id, title, created_at, updated_at, owner, dirty) VALUES (?, ?, ?, ?, ?, 0)
               ON CONFLICT(id) DO UPDATE SET
                 title = CASE WHEN nyra_conversations.dirty = 1 THEN nyra_conversations.title ELSE excluded.title END,
                 updated_at = MAX(nyra_conversations.updated_at, excluded.updated_at)
               WHERE nyra_conversations.owner IS excluded.owner""",
            (chat["id"], (chat.get("title") or "New chat")[:120], chat["createdAt"], chat["updatedAt"], owner),
        )
        for m in messages:
            await db.execute(
                """INSERT INTO nyra_messages (id, conversation_id, seq, role, created_at, payload, synced)
                   VALUES (?, ?, ?, ?, ?, ?, 1)
                   ON CONFLICT(id) DO UPDATE SET payload = excluded.payload, seq = excluded.seq
                   WHERE nyra_messages.synced = 1""",
                (m["id"], chat["id"], m["seq"], m["role"], m["createdAt"], m["payload"]),
            )
        await db.commit()

    async def set_title_from_remote(self, conversation_id: str, title: str) -> None:
        await self._conn().execute(
            "UPDATE nyra_conversations SET title = ? WHERE id = ? AND dirty = 0", (title[:120], conversation_id)
        )
        await self._conn().commit()

    async def mark_all_unsent(self, owner: str) -> None:
        """After the account's online copy was wiped: everything here counts as not sent."""
        db = self._conn()
        await db.execute("UPDATE nyra_conversations SET dirty = 1 WHERE owner = ?", (owner,))
        await db.execute(
            """UPDATE nyra_messages SET synced = 0
               WHERE conversation_id IN (SELECT id FROM nyra_conversations WHERE owner = ?)""",
            (owner,),
        )
        await db.execute("DELETE FROM nyra_deleted WHERE owner = ?", (owner,))
        await db.commit()
