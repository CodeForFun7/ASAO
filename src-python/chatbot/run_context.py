"""Live state of one user turn: the event stream, the activity tree and confirmations.

Tools and agents find the current run through context variables, which asyncio copies into
every task. That is how parallel tool calls and nested sub-agents all report to the same
live feed, each under the right parent.
"""

import asyncio
import contextvars
import time
import uuid
from typing import Any

from .config import CONFIRMATION_TIMEOUT_SECONDS

current_run: contextvars.ContextVar["RunContext | None"] = contextvars.ContextVar("nyra_run", default=None)
# Activity id that new activities nest under (a sub-agent while it works).
current_parent: contextvars.ContextVar[str | None] = contextvars.ContextVar("nyra_parent", default=None)

MAX_PROGRESS_LINES = 40


def now_ms() -> int:
    return int(time.time() * 1000)


class RunCancelled(BaseException):
    """The user pressed Stop.

    A BaseException (like asyncio.CancelledError) so ToolNode's error handling, which
    turns ordinary exceptions into tool messages, lets it through and the run stops.
    """


class RunContext:
    def __init__(self, conversation_id: str, user_request: str):
        self.run_id = uuid.uuid4().hex
        self.conversation_id = conversation_id
        self.user_request = user_request
        self.assistant_message_id = uuid.uuid4().hex
        self.started_at = now_ms()
        self.queue: asyncio.Queue[dict[str, Any] | None] = asyncio.Queue()
        self.cancelled = asyncio.Event()
        self.activities: dict[str, dict[str, Any]] = {}
        self.plan: list[dict[str, Any]] = []
        self.text = ""
        self._confirmations: dict[str, asyncio.Future[bool]] = {}

    # -- events --------------------------------------------------------------

    def emit(self, event_type: str, **data: Any) -> None:
        self.queue.put_nowait({"type": event_type, **data})

    def close(self) -> None:
        self.queue.put_nowait(None)

    def check_cancelled(self) -> None:
        if self.cancelled.is_set():
            raise RunCancelled()

    def cancel(self) -> None:
        self.cancelled.set()
        for fut in self._confirmations.values():
            if not fut.done():
                fut.set_result(False)

    # -- answer text -----------------------------------------------------------

    def append_text(self, delta: str) -> None:
        self.text += delta
        self.emit("token", text=delta)

    def commit_interim_text(self) -> None:
        """Text streamed before tool calls is narration, not the answer: move it to a note."""
        text = self.text.strip()
        self.text = ""
        self.emit("text_reset")
        if text:
            node_id = self.start_activity(kind="note", name="note", label=text)
            self.update_activity(node_id, status="done", endedAt=now_ms())

    # -- activities ------------------------------------------------------------

    def start_activity(self, *, kind: str, name: str, label: str, args: Any = None, parent_id: str | None = None) -> str:
        node_id = uuid.uuid4().hex[:16]
        node = {
            "id": node_id,
            "parentId": parent_id if parent_id is not None else current_parent.get(),
            "kind": kind,
            "name": name,
            "label": label,
            "status": "running",
            "args": args,
            "progress": [],
            "startedAt": now_ms(),
        }
        self.activities[node_id] = node
        self.emit("activity", node=node)
        return node_id

    def update_activity(self, node_id: str, **patch: Any) -> None:
        node = self.activities.get(node_id)
        if node is None:
            return
        node.update(patch)
        self.emit("activity_update", id=node_id, patch=patch)

    def progress(self, node_id: str, message: str) -> None:
        node = self.activities.get(node_id)
        if node is None or not message:
            return
        lines = node["progress"]
        lines.append(message)
        del lines[:-MAX_PROGRESS_LINES]
        self.emit("activity_progress", id=node_id, message=message)

    def set_plan(self, items: list[dict[str, Any]]) -> None:
        self.plan = items
        self.emit("plan", items=items)

    # -- confirmations ---------------------------------------------------------

    async def confirm(self, node_id: str, request: dict[str, Any]) -> bool:
        """Shows a confirmation card on the activity and waits for the user's answer."""
        if self.cancelled.is_set():
            return False
        confirmation_id = uuid.uuid4().hex
        fut: asyncio.Future[bool] = asyncio.get_running_loop().create_future()
        self._confirmations[confirmation_id] = fut
        card = {"id": confirmation_id, "status": "pending", **request}
        self.update_activity(node_id, status="waiting", confirmation=card)
        try:
            approved = await asyncio.wait_for(fut, timeout=CONFIRMATION_TIMEOUT_SECONDS)
        except asyncio.TimeoutError:
            approved = False
        finally:
            self._confirmations.pop(confirmation_id, None)
        card = {**card, "status": "approved" if approved else "denied"}
        self.update_activity(node_id, status="running", confirmation=card)
        self.check_cancelled()
        return approved

    def resolve_confirmation(self, confirmation_id: str, approved: bool) -> bool:
        fut = self._confirmations.get(confirmation_id)
        if fut is None or fut.done():
            return False
        fut.set_result(approved)
        return True

    # -- transcript ------------------------------------------------------------

    def transcript(self, status: str) -> dict[str, Any]:
        return {
            "status": status,
            "startedAt": self.started_at,
            "endedAt": now_ms(),
            "activities": list(self.activities.values()),
            "plan": self.plan,
        }


# Runs in progress, by run id, so confirm/stop requests can find them.
RUNS: dict[str, RunContext] = {}
