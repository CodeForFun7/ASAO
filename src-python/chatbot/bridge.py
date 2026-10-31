"""Client for the Rust tool bridge (src-tauri/src/bridge.rs).

Every system operation the agent performs goes through `call()`. The Rust app writes its
port and a per-launch token to `nyra-bridge.json`; this module re-reads that file when it
changes, so restarting ASAO does not require restarting the agent service.
"""

import json
from typing import Any

import httpx

from .config import bridge_file


class BridgeError(Exception):
    """A tool failed, or ASAO could not be reached."""


_client: httpx.AsyncClient | None = None
_cached: tuple[float, dict[str, Any]] | None = None


def _discovery() -> dict[str, Any]:
    global _cached
    path = bridge_file()
    try:
        mtime = path.stat().st_mtime
    except OSError as exc:
        raise BridgeError("ASAO is not running, so Nyra cannot reach your system. Start ASAO and try again.") from exc
    if _cached and _cached[0] == mtime:
        return _cached[1]
    try:
        info = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise BridgeError(f"The ASAO bridge file is unreadable: {exc}") from exc
    _cached = (mtime, info)
    return info


def _http() -> httpx.AsyncClient:
    global _client
    if _client is None:
        _client = httpx.AsyncClient(
            timeout=httpx.Timeout(connect=5.0, read=900.0, write=30.0, pool=30.0),
            limits=httpx.Limits(max_connections=64, max_keepalive_connections=16),
        )
    return _client


async def call(tool: str, args: dict[str, Any] | None = None, operation_id: str | None = None) -> Any:
    """Runs one bridge tool and returns its `data`. Raises BridgeError with a readable message."""
    info = _discovery()
    headers = {"X-Asao-Token": str(info.get("token", ""))}
    if operation_id:
        headers["X-Asao-Operation"] = operation_id
    url = f"http://127.0.0.1:{info.get('port')}/tool/{tool}"
    try:
        response = await _http().post(url, json=args or {}, headers=headers)
    except httpx.HTTPError as exc:
        raise BridgeError("ASAO is not responding. Make sure the ASAO app is running.") from exc
    try:
        payload = response.json()
    except ValueError as exc:
        raise BridgeError(f"ASAO returned an unreadable answer (HTTP {response.status_code}).") from exc
    if response.status_code == 401:
        raise BridgeError("ASAO rejected the agent's token. Restart ASAO and try again.")
    if not payload.get("ok"):
        raise BridgeError(str(payload.get("error") or "The operation failed."))
    return payload.get("data")


async def available() -> bool:
    try:
        info = _discovery()
        response = await _http().get(
            f"http://127.0.0.1:{info.get('port')}/health",
            headers={"X-Asao-Token": str(info.get("token", ""))},
            timeout=3.0,
        )
        return response.status_code == 200
    except (BridgeError, httpx.HTTPError):
        return False


async def close() -> None:
    global _client
    if _client is not None:
        await _client.aclose()
        _client = None
