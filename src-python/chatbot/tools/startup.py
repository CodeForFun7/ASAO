"""Apps and services that start with Windows."""

from typing import Any, Literal

from pydantic import BaseModel, Field

from ..formatting import clamp, human_bytes, lower, plural, shorten
from .base import Call, Result, cached, declined, invalidate, nyra_tool

# Sources ASAO cannot change (it refuses them to keep Windows bootable).
LOCKED_SOURCES = {"winlogon-shell", "winlogon-userinit", "winlogon-notify", "boot-execute", "app-init"}


async def startup_items(call: Call, refresh: bool = False) -> list[dict[str, Any]]:
    if refresh:
        invalidate("startup:")
    return await cached("startup:items", 45, lambda: call.bridge("startup.list"))


def compact(s: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": s["id"],
        "name": s["name"],
        "enabled": s.get("isEnabled"),
        "runningNow": s.get("isCurrentlyRunning") or None,
        "impact": s.get("impact"),
        "recommendation": s.get("recommendation"),
        "classification": s.get("classification"),
        "usage": s.get("usageFrequency"),
        "publisher": s.get("publisher"),
        "group": s.get("startupGroup"),
        "source": s.get("source"),
        "bootDelayMs": s.get("bootDurationMs") or None,
        "memory": human_bytes(s["memoryBytes"]) if s.get("memoryBytes") else None,
        "command": shorten(s.get("commandLine") or s.get("executablePath"), 140),
    }


def find_item(items: list[dict[str, Any]], item_id: str | None, name: str | None) -> dict[str, Any] | None:
    if item_id:
        for s in items:
            if s["id"] == item_id:
                return s
    if name:
        q = lower(name)
        exact = [s for s in items if lower(s["name"]) == q]
        if exact:
            return exact[0]
        partial = [s for s in items if q in lower(s["name"])]
        if len(partial) == 1:
            return partial[0]
    return None


class StartupListArgs(BaseModel):
    status: Literal["all", "enabled", "disabled"] = "all"
    recommendation: Literal["keep", "investigate", "disable"] | None = None
    impact: Literal["low", "medium", "high"] | None = None
    name_contains: str | None = None
    include_services: bool = Field(False, description="Include Windows services (many, mostly system).")
    refresh: bool = Field(False, description="Rescan instead of using a result from the last 45 seconds.")
    limit: int = 60


@nyra_tool(
    name="get_startup_list",
    description="List programs, scheduled tasks and services that start with Windows, with impact, usage and ASAO's keep/investigate/disable recommendation.",
    args=StartupListArgs,
    label=lambda a: "Reading startup apps",
)
async def get_startup_list(a: StartupListArgs, call: Call) -> Result:
    items = await startup_items(call, a.refresh)
    rows = items
    if not a.include_services:
        rows = [s for s in rows if s.get("source") != "windows-service"]
    if a.status != "all":
        rows = [s for s in rows if bool(s.get("isEnabled")) == (a.status == "enabled")]
    if a.recommendation:
        rows = [s for s in rows if s.get("recommendation") == a.recommendation]
    if a.impact:
        rows = [s for s in rows if s.get("impact") == a.impact]
    if a.name_contains:
        rows = [s for s in rows if lower(a.name_contains) in lower(s["name"])]
    order = {"high": 0, "medium": 1, "low": 2}
    rows = sorted(rows, key=lambda s: (not s.get("isEnabled"), order.get(s.get("impact"), 3), lower(s["name"])))
    enabled = [s for s in items if s.get("isEnabled")]
    totals = {
        "total": len(items),
        "enabled": len(enabled),
        "highImpactEnabled": sum(1 for s in enabled if s.get("impact") == "high"),
        "recommendedToDisable": sum(1 for s in enabled if s.get("recommendation") == "disable"),
        "services": sum(1 for s in items if s.get("source") == "windows-service"),
    }
    return Result(
        {"totals": totals, "items": [compact(s) for s in rows[: clamp(a.limit, 60, 1, 200)]]},
        f"{plural(totals['enabled'], 'enabled entry', 'enabled entries')}, {totals['recommendedToDisable']} recommended to disable",
    )


class StartupArgs(BaseModel):
    id: str | None = Field(None, description="Startup item id from get_startup_list.")
    name: str | None = None


@nyra_tool(
    name="get_startup",
    description="Full details of one startup entry: command, location, boot cost, what disabling it does and how to restore it.",
    args=StartupArgs,
    label=lambda a: f"Inspecting startup entry {a.name or a.id}",
)
async def get_startup(a: StartupArgs, call: Call) -> Result:
    item = find_item(await startup_items(call), a.id, a.name)
    if not item:
        return Result({"found": False, "message": "No startup entry matches. Use get_startup_list to find the id."}, "Not found", "error")
    data = {
        **compact(item),
        "executablePath": item.get("executablePath"),
        "commandLine": item.get("commandLine"),
        "description": item.get("description"),
        "pid": item.get("pid"),
        "bootCpuMs": item.get("bootCpuMs"),
        "bootDisk": human_bytes(item.get("bootDiskBytes")),
        "disableMethod": item.get("disableMethod"),
        "disableConsequences": item.get("disableConsequences"),
        "restoreMethod": item.get("restoreMethod"),
        "registryKey": item.get("registryKey"),
        "serviceName": item.get("serviceName"),
        "taskPath": item.get("taskPath"),
        "canChange": item.get("source") not in LOCKED_SOURCES,
    }
    return Result(data, f"{item['name']}: {item.get('impact')} impact, {'enabled' if item.get('isEnabled') else 'disabled'}")


class ToggleArgs(BaseModel):
    id: str = Field(description="Startup item id from get_startup_list.")
    reason: str = Field("", description="Why, in one sentence. Shown to the user.")


async def _toggle(a: ToggleArgs, call: Call, enable: bool) -> Result:
    item = find_item(await startup_items(call), a.id, a.id)
    if not item:
        return Result({"error": "No startup entry has this id. Use get_startup_list first."}, "Not found", "error")
    if bool(item.get("isEnabled")) == enable:
        return Result({"changed": False, "message": f"{item['name']} is already {'enabled' if enable else 'disabled'}."}, "Already in that state")
    if item.get("source") in LOCKED_SOURCES:
        return Result({"changed": False, "message": "This is a core Windows boot component; ASAO does not change it."}, "Protected", "error")

    essential = item.get("classification") == "essential" or item.get("source") == "windows-service"
    if enable:
        title = f"Start {item['name']} with Windows again?"
        message = f"{a.reason} It will launch automatically at your next sign-in.".strip()
    else:
        title = f"Stop {item['name']} from starting with Windows?"
        message = " ".join(x for x in [a.reason, item.get("disableConsequences"), f"To undo: {item.get('restoreMethod')}" if item.get("restoreMethod") else None] if x)
    ok = await call.confirm(
        title=title,
        message=message,
        items=[{"label": item["name"], "detail": f"{item.get('startupGroup')} · {item.get('impact')} impact"}],
        confirm_label="Enable" if enable else "Disable",
        danger="high" if essential and not enable else "medium",
    )
    if not ok:
        return declined(("enabling " if enable else "disabling ") + item["name"])

    await call.bridge(
        "startup.enable" if enable else "startup.disable",
        {
            "itemId": item["id"],
            "source": item["source"],
            "name": item["name"],
            "registryKey": item.get("registryKey"),
            "serviceName": item.get("serviceName"),
            "taskPath": item.get("taskPath"),
            "commandLine": item.get("commandLine"),
        },
    )
    invalidate("startup:")
    verb = "enabled" if enable else "disabled"
    return Result({"changed": True, "name": item["name"], "message": f"{item['name']} is now {verb} at startup."}, f"{item['name']} {verb}")


@nyra_tool(
    name="disable_Autostart",
    description="Stop a startup entry from launching with Windows (reversible). The user confirms in the chat first.",
    args=ToggleArgs,
    risk="change",
    label=lambda a: f"Disabling startup entry {a.id}",
)
async def disable_autostart(a: ToggleArgs, call: Call) -> Result:
    return await _toggle(a, call, enable=False)


@nyra_tool(
    name="enable_Autostart",
    description="Let a disabled startup entry launch with Windows again. The user confirms in the chat first.",
    args=ToggleArgs,
    risk="change",
    label=lambda a: f"Enabling startup entry {a.id}",
)
async def enable_autostart(a: ToggleArgs, call: Call) -> Result:
    return await _toggle(a, call, enable=True)


class NoArgs(BaseModel):
    pass


@nyra_tool(
    name="get_boot_performance",
    description="Boot performance: last boot time, uptime, estimated delay from startup items, high-impact count and boot phases.",
    args=NoArgs,
    label=lambda a: "Measuring boot performance",
)
async def get_boot_performance(a: NoArgs, call: Call) -> Result:
    s = await call.bridge("startup.boot")
    data = {
        "lastBootTime": s.get("lastBootTime"),
        "uptimeHours": round((s.get("uptimeSeconds") or 0) / 3600, 1),
        "startupItems": s.get("totalStartupItems"),
        "highImpact": s.get("highImpactCount"),
        "estimatedBootDelaySeconds": round((s.get("estimatedBootDelayMs") or 0) / 1000, 1),
        "memoryUsedByStartupApps": human_bytes(s.get("totalMemoryImpactBytes")),
        "phases": [{"name": p["name"], "seconds": round(p["durationMs"] / 1000, 1)} for p in s.get("bootPhases", [])],
        "traceStatus": s.get("etwStatus"),
    }
    return Result(data, f"Estimated startup delay {data['estimatedBootDelaySeconds']} s, {data['highImpact']} high-impact items")


TOOLS = [get_startup_list, get_startup, disable_autostart, enable_autostart, get_boot_performance]
