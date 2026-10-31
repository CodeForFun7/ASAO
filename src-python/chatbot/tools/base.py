"""How Nyra tools are declared.

A tool is an async function `fn(args, call) -> Result`, wrapped by `nyra_tool` into a
LangChain StructuredTool. The wrapper:
- adds the call to the live activity feed (with a readable label) and records its outcome,
- gives the function a `Call` for progress lines, user confirmation and bridge access,
- turns errors into a JSON error the model can react to,
- keeps the result small enough for the model (see formatting.compact_json).
"""

import asyncio
import time
from dataclasses import dataclass
from typing import Any, Awaitable, Callable, Literal, TypeVar

from langchain_core.tools import StructuredTool
from pydantic import BaseModel

from .. import bridge
from ..formatting import compact_json
from ..run_context import RunCancelled, current_run

Risk = Literal["read", "change", "destructive"]
A = TypeVar("A", bound=BaseModel)


@dataclass
class Result:
    data: Any
    summary: str
    status: Literal["done", "denied", "error"] = "done"


class Call:
    """What a running tool can do besides computing its result."""

    def __init__(self, activity_id: str | None):
        self.activity_id = activity_id
        self.run = current_run.get()

    def progress(self, message: str) -> None:
        if self.run and self.activity_id:
            self.run.progress(self.activity_id, message)

    async def confirm(
        self,
        *,
        title: str,
        message: str,
        items: list[dict[str, str]] | None = None,
        total_items: int | None = None,
        confirm_label: str = "Confirm",
        danger: Literal["medium", "high"] = "medium",
    ) -> bool:
        """Asks the user in the chat. False when declined, timed out, or there is no live chat."""
        if not (self.run and self.activity_id):
            return False
        return await self.run.confirm(
            self.activity_id,
            {
                "title": title,
                "message": message,
                "items": items or [],
                "totalItems": total_items if total_items is not None else len(items or []),
                "confirmLabel": confirm_label,
                "danger": danger,
            },
        )

    async def bridge(self, tool: str, args: dict[str, Any] | None = None) -> Any:
        """Calls ASAO. Long operations stream progress to this tool's activity."""
        return await bridge.call(tool, args, operation_id=self.activity_id)


def declined(what: str) -> Result:
    return Result(
        data={
            "status": "declined",
            "message": f"The user declined: {what}. Nothing was changed. Do not retry unless the user asks again.",
        },
        summary="You declined, nothing was changed",
        status="denied",
    )


def nyra_tool(
    *,
    name: str,
    description: str,
    args: type[A],
    label: Callable[[A], str],
    risk: Risk = "read",
    kind: Literal["tool", "agent"] = "tool",
    hidden: bool = False,
) -> Callable[[Callable[[A, Call], Awaitable[Result]]], StructuredTool]:
    def wrap(fn: Callable[[A, Call], Awaitable[Result]]) -> StructuredTool:
        async def run(**kwargs: Any) -> str:
            parsed = args(**kwargs)
            ctx = current_run.get()
            activity_id = None
            if ctx and not hidden:
                ctx.check_cancelled()
                try:
                    text = label(parsed)
                except Exception:  # a label must never break the tool
                    text = name
                activity_id = ctx.start_activity(
                    kind=kind, name=name, label=text, args=parsed.model_dump(exclude_none=True)
                )
            call = Call(activity_id)
            started = time.monotonic()
            try:
                result = await fn(parsed, call)
            except (RunCancelled, asyncio.CancelledError):
                if ctx and activity_id:
                    ctx.update_activity(activity_id, status="cancelled", error="Stopped by you.", endedAt=_now())
                raise
            except Exception as exc:  # reported to the model, which decides what to do
                message = str(exc) or exc.__class__.__name__
                if ctx and activity_id:
                    ctx.update_activity(activity_id, status="error", error=message, endedAt=_now())
                return compact_json({"error": message})
            if ctx and activity_id:
                ctx.update_activity(
                    activity_id,
                    status=result.status,
                    summary=result.summary,
                    endedAt=_now(),
                    durationMs=int((time.monotonic() - started) * 1000),
                )
            return compact_json(result.data)

        return StructuredTool.from_function(
            coroutine=run,
            name=name,
            description=description,
            args_schema=args,
            metadata={"risk": risk, "kind": kind},
        )

    return wrap


def _now() -> int:
    return int(time.time() * 1000)


# ---------------------------------------------------------------------------
# Shared caches so sibling tools in one turn do not repeat slow scans
# ---------------------------------------------------------------------------

_cache: dict[str, tuple[float, Any]] = {}
_locks: dict[str, asyncio.Lock] = {}


async def cached(key: str, ttl_seconds: float, factory: Callable[[], Awaitable[Any]]) -> Any:
    """Single-flight cache: concurrent callers with the same key share one computation."""
    lock = _locks.setdefault(key, asyncio.Lock())
    async with lock:
        hit = _cache.get(key)
        if hit and time.monotonic() - hit[0] < ttl_seconds:
            return hit[1]
        value = await factory()
        _cache[key] = (time.monotonic(), value)
        return value


def invalidate(prefix: str) -> None:
    for key in [k for k in _cache if k.startswith(prefix)]:
        _cache.pop(key, None)


async def gather_limited(items: list[Any], limit: int, fn: Callable[[Any], Awaitable[Any]]) -> list[Any]:
    sem = asyncio.Semaphore(limit)

    async def one(item: Any) -> Any:
        async with sem:
            return await fn(item)

    return await asyncio.gather(*(one(i) for i in items))
