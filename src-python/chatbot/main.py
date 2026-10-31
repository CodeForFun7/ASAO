"""Nyra agent service (FastAPI on 127.0.0.1:18743).

Endpoints (all but /health need the `X-Nyra-Client: asao` header; a custom header forces a
CORS preflight, so only the ASAO window can call them from a browser engine):

  GET    /health                          service and ASAO bridge status
  POST   /chat/stream                     run one turn; Server-Sent Events stream of the live feed
  POST   /chat/{run_id}/confirm           answer a confirmation card
  POST   /chat/{run_id}/stop              stop a running turn
  GET    /conversations                   saved conversations
  GET    /conversations/{id}              one conversation with its messages
  PATCH  /conversations/{id}              rename
  DELETE /conversations/{id}              delete (chat transcript and model state)

Stream events (JSON in `data:` lines): run, token, text_reset, activity, activity_update,
activity_progress, plan, done. The `done` event carries the saved assistant message.
"""

import asyncio
import json
import re
import uuid
from contextlib import asynccontextmanager
from typing import Any, AsyncIterator

import openai
from fastapi import Depends, FastAPI, Header, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import StreamingResponse
from langgraph.checkpoint.sqlite.aio import AsyncSqliteSaver
from langgraph.errors import GraphRecursionError
from pydantic import BaseModel, Field, field_validator

from . import bridge
from .bridge import BridgeError
from .config import DEFAULT_MODEL, database_file
from .graph import LlmSettings, run_turn
from .history import ConversationStore
from .run_context import RUNS, RunCancelled, RunContext, current_run, now_ms

ID_PATTERN = re.compile(r"^[A-Za-z0-9_-]{1,64}$")

store = ConversationStore(database_file())
TASKS: dict[str, asyncio.Task[None]] = {}
_saver: AsyncSqliteSaver | None = None


@asynccontextmanager
async def lifespan(_: FastAPI) -> AsyncIterator[None]:
    global _saver
    await store.open()
    async with AsyncSqliteSaver.from_conn_string(str(database_file())) as saver:
        await saver.setup()
        _saver = saver
        try:
            yield
        finally:
            for run in list(RUNS.values()):
                run.cancel()
            await store.close()
            await bridge.close()
            _saver = None


app = FastAPI(title="Nyra agent", lifespan=lifespan)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:1420", "http://tauri.localhost", "https://tauri.localhost"],
    allow_methods=["GET", "POST", "PATCH", "DELETE", "OPTIONS"],
    allow_headers=["content-type", "x-nyra-client"],
)


def client_header(x_nyra_client: str | None = Header(default=None)) -> None:
    if x_nyra_client != "asao":
        raise HTTPException(status_code=403, detail="Requests must come from ASAO.")


def valid_id(value: str) -> str:
    if not ID_PATTERN.match(value):
        raise HTTPException(status_code=400, detail="Invalid id.")
    return value


# ---------------------------------------------------------------------------
# Health
# ---------------------------------------------------------------------------


@app.get("/health")
async def health() -> dict[str, Any]:
    return {"ok": True, "bridge": await bridge.available()}


# ---------------------------------------------------------------------------
# Chat
# ---------------------------------------------------------------------------


class ChatRequest(BaseModel):
    api_key: str = Field(min_length=1, max_length=300)
    model: str = Field(DEFAULT_MODEL, min_length=1, max_length=100)
    base_url: str | None = Field(None, max_length=300)
    conversation_id: str | None = None
    message: str = Field(min_length=1, max_length=12_000)

    @field_validator("base_url")
    @classmethod
    def _base_url(cls, v: str | None) -> str | None:
        if v is None or not v.strip():
            return None
        if not re.match(r"^https?://", v.strip()):
            raise ValueError("base_url must start with http:// or https://")
        return v.strip()

    @field_validator("conversation_id")
    @classmethod
    def _conversation_id(cls, v: str | None) -> str | None:
        if v is not None and not ID_PATTERN.match(v):
            raise ValueError("Invalid conversation id")
        return v


def friendly_error(exc: BaseException) -> str:
    if isinstance(exc, openai.AuthenticationError):
        return "The API key was rejected. Check it in Nyra settings."
    if isinstance(exc, openai.RateLimitError):
        if "insufficient_quota" in str(exc):
            return "The API account has no remaining quota."
        return "The model provider is rate limiting requests. Try again in a moment."
    if isinstance(exc, openai.NotFoundError):
        return "The model or endpoint was not found. Check the model name in Nyra settings."
    if isinstance(exc, openai.APIConnectionError):
        return "Could not reach the model provider. Check the internet connection."
    if isinstance(exc, openai.APIError):
        return f"The model provider returned an error: {exc.message}"
    if isinstance(exc, BridgeError):
        return str(exc)
    if isinstance(exc, GraphRecursionError):
        return "The request needed too many steps. Try splitting it into smaller requests."
    return f"Nyra hit an unexpected error: {exc}"


async def execute(run: RunContext, settings: LlmSettings) -> None:
    current_run.set(run)
    status, error, content = "done", None, ""
    try:
        content = await run_turn(run, settings, _saver)
    except (asyncio.CancelledError, RunCancelled):
        status, content = "cancelled", run.text
    except Exception as exc:  # reported in the chat
        status, error, content = "error", friendly_error(exc), run.text
    finally:
        for node in run.activities.values():
            if node["status"] in ("running", "waiting"):
                node.update(status="cancelled" if status != "error" else "error", endedAt=now_ms())
                run.emit("activity_update", id=node["id"], patch={"status": node["status"], "endedAt": node["endedAt"]})
        message = {
            "id": run.assistant_message_id,
            "role": "assistant",
            "content": content,
            "createdAt": run.started_at,
            "run": run.transcript(status),
            "error": error,
        }
        try:
            await store.save_message(run.conversation_id, "", message)
        except Exception as exc:  # the answer is still delivered
            message["error"] = (error + " " if error else "") + f"(Could not save this message: {exc})"
        run.emit("done", message=message)
        run.close()
        RUNS.pop(run.run_id, None)
        TASKS.pop(run.run_id, None)


async def event_stream(run: RunContext, first: dict[str, Any]) -> AsyncIterator[str]:
    yield f"data: {json.dumps(first)}\n\n"
    while True:
        try:
            event = await asyncio.wait_for(run.queue.get(), timeout=15)
        except asyncio.TimeoutError:
            yield ": keep-alive\n\n"
            continue
        if event is None:
            break
        yield f"data: {json.dumps(event, ensure_ascii=False, default=str)}\n\n"


@app.post("/chat/stream", dependencies=[Depends(client_header)])
async def chat_stream(body: ChatRequest) -> StreamingResponse:
    conversation_id = body.conversation_id or uuid.uuid4().hex
    if any(r.conversation_id == conversation_id for r in RUNS.values()):
        raise HTTPException(status_code=409, detail="Nyra is still working on this conversation.")
    text = body.message.strip()
    run = RunContext(conversation_id, text)
    user_message = {"id": uuid.uuid4().hex, "role": "user", "content": text, "createdAt": now_ms()}
    title = text.splitlines()[0][:60]
    await store.save_message(conversation_id, title, user_message)

    RUNS[run.run_id] = run
    settings = LlmSettings(api_key=body.api_key.strip(), model=body.model.strip(), base_url=body.base_url)
    # The run continues if the window stops listening; its result is saved either way.
    TASKS[run.run_id] = asyncio.create_task(execute(run, settings))
    first = {
        "type": "run",
        "runId": run.run_id,
        "conversationId": conversation_id,
        "title": title,
        "userMessage": user_message,
        "assistantMessageId": run.assistant_message_id,
        "startedAt": run.started_at,
    }
    return StreamingResponse(
        event_stream(run, first),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


class ConfirmRequest(BaseModel):
    confirmation_id: str
    approved: bool


@app.post("/chat/{run_id}/confirm", dependencies=[Depends(client_header)])
async def chat_confirm(run_id: str, body: ConfirmRequest) -> dict[str, Any]:
    run = RUNS.get(valid_id(run_id))
    if run is None:
        raise HTTPException(status_code=404, detail="This request has already finished.")
    if not run.resolve_confirmation(body.confirmation_id, body.approved):
        raise HTTPException(status_code=404, detail="This confirmation is no longer pending.")
    return {"ok": True}


@app.post("/chat/{run_id}/stop", dependencies=[Depends(client_header)])
async def chat_stop(run_id: str) -> dict[str, Any]:
    run = RUNS.get(valid_id(run_id))
    if run is None:
        return {"ok": True, "alreadyFinished": True}
    run.cancel()
    task = TASKS.get(run_id)
    if task and not task.done():
        task.cancel()
    return {"ok": True}


# ---------------------------------------------------------------------------
# Conversations
# ---------------------------------------------------------------------------


@app.get("/conversations", dependencies=[Depends(client_header)])
async def list_conversations(limit: int = 100) -> list[dict[str, Any]]:
    rows = await store.list(limit)
    active = {r.conversation_id for r in RUNS.values()}
    return [{**r, "running": r["id"] in active} for r in rows]


@app.get("/conversations/{conversation_id}", dependencies=[Depends(client_header)])
async def get_conversation(conversation_id: str) -> dict[str, Any]:
    record = await store.get(valid_id(conversation_id))
    if record is None:
        raise HTTPException(status_code=404, detail="Conversation not found.")
    return record


class RenameRequest(BaseModel):
    title: str = Field(min_length=1, max_length=120)


@app.patch("/conversations/{conversation_id}", dependencies=[Depends(client_header)])
async def rename_conversation(conversation_id: str, body: RenameRequest) -> dict[str, Any]:
    if not await store.rename(valid_id(conversation_id), body.title):
        raise HTTPException(status_code=404, detail="Conversation not found.")
    return {"ok": True}


@app.delete("/conversations/{conversation_id}", dependencies=[Depends(client_header)])
async def delete_conversation(conversation_id: str) -> dict[str, Any]:
    valid_id(conversation_id)
    for run in list(RUNS.values()):
        if run.conversation_id == conversation_id:
            run.cancel()
            task = TASKS.get(run.run_id)
            if task:
                task.cancel()
    await store.delete(conversation_id)
    if _saver is not None:
        await _saver.adelete_thread(conversation_id)
    return {"ok": True}
