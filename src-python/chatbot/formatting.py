"""Small helpers that turn raw tool data into compact, model-friendly values."""

import json
import time
from datetime import datetime, timezone
from typing import Any

from .config import MAX_TOOL_RESULT_CHARS

DAY_MS = 86_400_000
MB = 1024 * 1024


def human_bytes(n: int | float | None) -> str:
    if n is None:
        return "unknown"
    n = float(n)
    if n <= 0:
        return "0 B"
    for unit, size in (("TB", 1024**4), ("GB", 1024**3), ("MB", 1024**2), ("KB", 1024)):
        if n >= size:
            value = n / size
            return f"{value:.2f} {unit}" if value < 10 else f"{value:.1f} {unit}" if value < 100 else f"{value:.0f} {unit}"
    return f"{int(n)} B"


def iso_date(ms: int | None) -> str | None:
    if not ms:
        return None
    try:
        return datetime.fromtimestamp(ms / 1000, tz=timezone.utc).strftime("%Y-%m-%d")
    except (OverflowError, OSError, ValueError):
        return None


def now_ms() -> int:
    return int(time.time() * 1000)


def days_since(ms: int | None) -> int | None:
    if not ms:
        return None
    return max(0, (now_ms() - ms) // DAY_MS)


def plural(n: int, one: str, many: str | None = None) -> str:
    return f"{n:,} {one if n == 1 else (many or one + 's')}"


def shorten(text: str | None, limit: int = 140) -> str | None:
    if not text:
        return None
    return text if len(text) <= limit else text[: limit - 1] + "…"


def lower(s: str | None) -> str:
    return (s or "").lower()


def clamp(value: Any, default: int, low: int, high: int) -> int:
    try:
        n = int(value)
    except (TypeError, ValueError):
        return default
    return max(low, min(high, n))


def _trim(value: Any, keep: int) -> Any:
    if isinstance(value, list):
        head = [_trim(v, keep) for v in value[:keep]]
        if len(value) > keep:
            head.append(f"… {len(value) - keep} more not shown")
        return head
    if isinstance(value, dict):
        return {k: _trim(v, keep) for k, v in value.items()}
    if isinstance(value, str) and len(value) > 600:
        return value[:600] + "…"
    return value


def _drop_none(value: Any) -> Any:
    if isinstance(value, dict):
        return {k: _drop_none(v) for k, v in value.items() if v is not None}
    if isinstance(value, list):
        return [_drop_none(v) for v in value]
    return value


def compact_json(data: Any, max_chars: int = MAX_TOOL_RESULT_CHARS) -> str:
    """JSON for the model. Long arrays are shortened (with a note) until it fits."""
    data = _drop_none(data)
    full = json.dumps(data, ensure_ascii=False, default=str)
    if len(full) <= max_chars:
        return full
    for keep in (80, 40, 20, 10, 5, 2):
        text = json.dumps(_trim(data, keep), ensure_ascii=False, default=str)
        if len(text) <= max_chars:
            return text
    return full[:max_chars] + "… [result truncated]"
