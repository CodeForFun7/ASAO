"""Saved conversations.

Two layers live in the same SQLite file (`nyra.db` in ASAO's app data folder):
- LangGraph's AsyncSqliteSaver keeps each conversation's full model state (messages, tool
  calls and results) keyed by thread id = conversation id. That is what gives follow-up
  questions their context.
- This store keeps what the chat shows: titles, and each message with its live activity
  trace (tools, agents, confirmations, plan), so past conversations can be reopened.
"""

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
"""

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
        await self.db.commit()

    async def close(self) -> None:
        if self.db is not None:
            await self.db.close()
            self.db = None

    def _conn(self) -> aiosqlite.Connection:
        if self.db is None:
            raise RuntimeError("Conversation store is not open")
        return self.db

    async def list(self, limit: int = 100) -> list[dict[str, Any]]:
        cur = await self._conn().execute(
            """SELECT c.id, c.title, c.created_at, c.updated_at,
                      (SELECT COUNT(*) FROM nyra_messages m WHERE m.conversation_id = c.id)
               FROM nyra_conversations c ORDER BY c.updated_at DESC LIMIT ?""",
            (max(1, min(limit, 500)),),
        )
        rows = await cur.fetchall()
        return [
            {"id": r[0], "title": r[1], "createdAt": r[2], "updatedAt": r[3], "messageCount": r[4]}
            for r in rows
        ]

    async def get(self, conversation_id: str) -> dict[str, Any] | None:
        db = self._conn()
        cur = await db.execute(
            "SELECT id, title, created_at, updated_at FROM nyra_conversations WHERE id = ?", (conversation_id,)
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

    async def save_message(self, conversation_id: str, title: str, message: dict[str, Any]) -> None:
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
            """INSERT INTO nyra_conversations (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at""",
            (conversation_id, (title.strip() or "New chat")[:120], now, now),
        )
        await db.execute(
            """INSERT INTO nyra_messages (id, conversation_id, seq, role, created_at, payload)
               VALUES (?, ?, COALESCE((SELECT MAX(seq) + 1 FROM nyra_messages WHERE conversation_id = ?), 0), ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET payload = excluded.payload""",
            (message["id"], conversation_id, conversation_id, message.get("role", "assistant"), message.get("createdAt", now), payload),
        )
        await db.commit()

    async def rename(self, conversation_id: str, title: str) -> bool:
        cur = await self._conn().execute(
            "UPDATE nyra_conversations SET title = ? WHERE id = ?", (title.strip()[:120], conversation_id)
        )
        await self._conn().commit()
        return cur.rowcount > 0

    async def delete(self, conversation_id: str) -> None:
        db = self._conn()
        await db.execute("DELETE FROM nyra_messages WHERE conversation_id = ?", (conversation_id,))
        await db.execute("DELETE FROM nyra_conversations WHERE id = ?", (conversation_id,))
        await db.commit()
