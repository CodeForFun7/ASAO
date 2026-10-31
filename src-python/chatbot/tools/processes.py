"""Background processes and live system load."""

import asyncio
from typing import Any, Literal

from pydantic import BaseModel, Field

from ..formatting import MB, clamp, human_bytes, lower, plural, shorten
from .base import Call, Result, declined, nyra_tool

NEVER_END = {
    "system", "registry", "secure system", "memory compression", "smss.exe", "csrss.exe", "wininit.exe",
    "winlogon.exe", "services.exe", "lsass.exe", "lsaiso.exe", "svchost.exe", "dwm.exe", "fontdrvhost.exe",
    "msmpeng.exe", "audiodg.exe", "asao.exe",
}

Category = Literal["windows-core", "drivers", "gaming", "development", "productivity", "communication", "browser", "unknown"]


def can_end(p: dict[str, Any]) -> tuple[bool, str | None]:
    if p["pid"] <= 4 or p.get("isSystemCritical") or lower(p["name"]) in NEVER_END:
        return False, "Windows or ASAO process; ending it could crash the PC or ASAO."
    return True, None


def same_name(a: str, b: str) -> bool:
    return lower(a).removesuffix(".exe") == lower(b).removesuffix(".exe")


def compact(p: dict[str, Any]) -> dict[str, Any]:
    return {
        "pid": p["pid"],
        "name": p["name"],
        "product": p.get("productName"),
        "publisher": p.get("publisher"),
        "cpuPercent": round(p.get("cpuPercent", 0), 1),
        "memory": human_bytes(p.get("memoryBytes")),
        "diskPerSec": f"{human_bytes(p['diskBytesPerSec'])}/s" if p.get("diskBytesPerSec", 0) > 1024 else None,
        "networkPerSec": f"{human_bytes(p['networkBytesPerSec'])}/s" if p.get("networkBytesPerSec", 0) > 1024 else None,
        "category": p.get("category"),
        "status": p.get("status"),
        "activity": p.get("activityState"),
        "impact": p.get("impactLevel"),
        "startsWithWindows": p.get("isStartup") or None,
        "systemCritical": p.get("isSystemCritical") or None,
    }


def impact_score(p: dict[str, Any]) -> float:
    return (p.get("backgroundImpactScore") or 0) * 10 + p.get("cpuPercent", 0) * 2 + p.get("memoryBytes", 0) / (64 * MB)


SORT_KEYS = {
    "impact": impact_score,
    "cpu": lambda p: p.get("cpuPercent", 0),
    "memory": lambda p: p.get("memoryBytes", 0),
    "disk": lambda p: p.get("diskBytesPerSec", 0),
    "network": lambda p: p.get("networkBytesPerSec", 0),
}


async def list_processes(call: Call) -> list[dict[str, Any]]:
    return await call.bridge("processes.list")


class ProcessListArgs(BaseModel):
    sort_by: Literal["impact", "cpu", "memory", "disk", "network", "name"] = "impact"
    limit: int = Field(25, description="Max rows (max 150).")
    name_contains: str | None = None
    category: Category | None = None
    background_only: bool = Field(False, description="Only processes not in the foreground.")
    group_by_name: bool = Field(False, description="Combine instances of multi-process apps (browsers, Electron apps).")


@nyra_tool(
    name="get_processes_list",
    description="List running processes with CPU, memory, disk and network use, category, status and background impact.",
    args=ProcessListArgs,
    label=lambda a: f'Looking for processes named "{a.name_contains}"' if a.name_contains else f"Listing running processes by {a.sort_by}",
)
async def get_processes_list(a: ProcessListArgs, call: Call) -> Result:
    procs, metrics = await asyncio.gather(list_processes(call), call.bridge("system.metrics"))
    m = metrics["metrics"]
    rows = procs
    if a.name_contains:
        q = lower(a.name_contains)
        rows = [p for p in rows if q in lower(p["name"]) or q in lower(p.get("productName"))]
    if a.category:
        rows = [p for p in rows if p.get("category") == a.category]
    if a.background_only:
        rows = [p for p in rows if p.get("activityState") != "foreground"]
    limit = clamp(a.limit, 25, 1, 150)
    totals = {
        "processes": len(procs),
        "matching": len(rows),
        "cpuPercent": round(m["cpuUsagePercent"]),
        "memoryPercent": round(m["memoryUsagePercent"]),
        "memoryUsed": f"{human_bytes(m['memoryUsedBytes'])} of {human_bytes(m['memoryTotalBytes'])}",
        "systemStatus": m.get("systemStatus"),
        "needingAttention": sum(1 for p in procs if p.get("status") == "attention"),
        "highResource": sum(1 for p in procs if p.get("status") == "high-resource"),
    }

    if a.group_by_name:
        groups: dict[str, list[dict[str, Any]]] = {}
        for p in rows:
            groups.setdefault(lower(p["name"]), []).append(p)
        key = (lambda g: sum(p.get("cpuPercent", 0) for p in g)) if a.sort_by == "cpu" else (lambda g: sum(p.get("memoryBytes", 0) for p in g))
        out = [
            {
                "name": g[0]["name"],
                "product": g[0].get("productName"),
                "instances": len(g),
                "pids": [p["pid"] for p in g[:12]],
                "cpuPercent": round(sum(p.get("cpuPercent", 0) for p in g), 1),
                "memory": human_bytes(sum(p.get("memoryBytes", 0) for p in g)),
                "category": g[0].get("category"),
                "systemCritical": any(p.get("isSystemCritical") for p in g) or None,
            }
            for g in sorted(groups.values(), key=key, reverse=True)[:limit]
        ]
        return Result({"totals": totals, "groups": out}, f"{plural(len(procs), 'process', 'processes')} running, {len(groups)} distinct apps")

    if a.sort_by == "name":
        rows = sorted(rows, key=lambda p: lower(p["name"]))
    else:
        rows = sorted(rows, key=SORT_KEYS[a.sort_by], reverse=True)
    out = [compact(p) for p in rows[:limit]]
    top = out[0] if out else None
    summary = f"{plural(len(procs), 'process', 'processes')} running"
    if top:
        summary += f", top: {top['name']} ({top['cpuPercent']}% CPU, {top['memory']})"
    return Result({"totals": totals, "processes": out}, summary)


class ProcessArgs(BaseModel):
    pid: int | None = None
    name: str | None = Field(None, description="Process name such as chrome.exe; all instances are summarised.")


@nyra_tool(
    name="get_process",
    description="Full details of one process by PID or name: path, publisher, resource use, parent/child processes and whether it is safe to end.",
    args=ProcessArgs,
    label=lambda a: f"Inspecting process {a.pid if a.pid is not None else a.name}",
)
async def get_process(a: ProcessArgs, call: Call) -> Result:
    if a.pid is None and not a.name:
        raise ValueError("Give a pid or a name.")
    procs = await list_processes(call)
    matches = [p for p in procs if (p["pid"] == a.pid if a.pid is not None else same_name(p["name"], a.name or ""))]
    if not matches:
        return Result({"found": False, "message": "No running process matches."}, "Not running")
    by_pid = {p["pid"]: p for p in procs}

    def detail(p: dict[str, Any]) -> dict[str, Any]:
        allowed, reason = can_end(p)
        parent = by_pid.get(p.get("parentPid") or -1)
        return {
            **compact(p),
            "executablePath": p.get("executablePath"),
            "description": p.get("description"),
            "threads": p.get("threadCount"),
            "sustainedCpuPercent": p.get("sustainedCpuPercent"),
            "sustainedLoadSeconds": p.get("sustainedLoadSeconds"),
            "backgroundImpactScore": p.get("backgroundImpactScore"),
            "runningForMinutes": round(p["startedSecondsAgo"] / 60) if p.get("startedSecondsAgo") is not None else None,
            "restricted": p.get("isRestricted"),
            "parent": {"pid": p.get("parentPid"), "name": parent["name"] if parent else None} if p.get("parentPid") else None,
            "children": [{"pid": c["pid"], "name": c["name"]} for c in procs if c.get("parentPid") == p["pid"]][:20],
            "canEnd": allowed,
            "cannotEndReason": reason,
        }

    mem = sum(p.get("memoryBytes", 0) for p in matches)
    cpu = sum(p.get("cpuPercent", 0) for p in matches)
    return Result(
        {
            "found": True,
            "instances": len(matches),
            "totalMemory": human_bytes(mem),
            "totalCpuPercent": round(cpu, 1),
            "details": [detail(p) for p in matches[:8]],
            "otherPids": [p["pid"] for p in matches[8:]],
        },
        f"{matches[0]['name']}: {plural(len(matches), 'instance')}, {human_bytes(mem)}, {round(cpu)}% CPU",
    )


class EndTaskArgs(BaseModel):
    pids: list[int] = Field(default_factory=list)
    name: str | None = Field(None, description="End every process with this name, e.g. 'OneDrive.exe'.")
    reason: str = Field(description="Why, in one sentence. Shown to the user in the confirmation.")


@nyra_tool(
    name="end_background_task",
    description=(
        "End (terminate) processes by PID list and/or by name (all instances). Windows-critical processes are refused. "
        "The user confirms in the chat before anything ends. Unsaved work in those apps is lost."
    ),
    args=EndTaskArgs,
    risk="destructive",
    label=lambda a: f"Ending {a.name or ', '.join(map(str, a.pids))}",
)
async def end_background_task(a: EndTaskArgs, call: Call) -> Result:
    procs = await list_processes(call)
    wanted = set(a.pids)
    targets = [p for p in procs if p["pid"] in wanted or (a.name and same_name(p["name"], a.name))]
    missing = [pid for pid in wanted if pid not in {p["pid"] for p in procs}]
    if not targets:
        return Result({"ended": [], "message": "No running process matches.", "missingPids": missing}, "Nothing to end", "error")
    allowed = [p for p in targets if can_end(p)[0]]
    refused = [{"pid": p["pid"], "name": p["name"], "reason": can_end(p)[1]} for p in targets if not can_end(p)[0]]
    if not allowed:
        return Result({"ended": [], "refused": refused}, "Refused: protected processes", "error")

    ok = await call.confirm(
        title=f"End {plural(len(allowed), 'process', 'processes')}?",
        message=f"{a.reason} Unsaved work in these apps will be lost.",
        items=[
            {"label": f"{p['name']} (PID {p['pid']})", "detail": f"{round(p.get('cpuPercent', 0))}% CPU · {human_bytes(p.get('memoryBytes'))}"}
            for p in allowed[:40]
        ],
        total_items=len(allowed),
        confirm_label="End processes",
        danger="high" if len(allowed) > 5 else "medium",
    )
    if not ok:
        return declined("ending " + ", ".join(sorted({p["name"] for p in allowed})))

    call.progress(f"Ending {plural(len(allowed), 'process', 'processes')}")

    async def end(p: dict[str, Any]) -> dict[str, Any]:
        try:
            return await call.bridge("process.terminate", {"pid": p["pid"], "expectedName": p["name"]})
        except Exception as exc:  # one failure must not hide the others
            return {"pid": p["pid"], "name": p["name"], "status": "failed", "message": str(exc)}

    outcomes = await asyncio.gather(*(end(p) for p in allowed))
    ended = [o for o in outcomes if o.get("status") == "terminated"]
    failed = [o for o in outcomes if o.get("status") != "terminated"]
    freed = sum(p.get("memoryBytes", 0) for p in allowed if any(o["pid"] == p["pid"] for o in ended))
    summary = f"Ended {len(ended)} of {len(allowed)}"
    if failed:
        summary += f", {len(failed)} failed"
    if refused:
        summary += f", {len(refused)} refused"
    return Result(
        {"ended": ended, "failed": failed, "refused": refused, "memoryFreedApprox": human_bytes(freed)},
        summary,
        "done" if ended else "error",
    )


class NoArgs(BaseModel):
    pass


@nyra_tool(
    name="get_system_metrics",
    description="Current CPU, memory, GPU, disk and network load, system strain, process counts and battery status.",
    args=NoArgs,
    label=lambda a: "Reading system load and battery",
)
async def get_system_metrics(a: NoArgs, call: Call) -> Result:
    data = await call.bridge("system.metrics")
    m, b = data["metrics"], data.get("battery") or {}
    battery: Any = "no battery"
    if b.get("hasBattery"):
        battery = {
            "percent": b.get("percent"),
            "state": b.get("state"),
            "healthPercent": b.get("healthPercent"),
            "timeRemainingMin": round(b["timeRemainingSecs"] / 60) if b.get("timeRemainingSecs") else None,
            "batterySaver": b.get("batterySaver"),
        }
    return Result(
        {
            "status": m.get("systemStatus"),
            "cpuPercent": round(m["cpuUsagePercent"]),
            "memoryPercent": round(m["memoryUsagePercent"]),
            "memory": f"{human_bytes(m['memoryUsedBytes'])} of {human_bytes(m['memoryTotalBytes'])}",
            "gpuPercent": round(m.get("gpuUsagePercent", 0)),
            "diskPerSec": f"{human_bytes(m['diskBytesPerSec'])}/s" if m.get("diskBytesPerSec") is not None else None,
            "networkPerSec": f"{human_bytes(m['networkBytesPerSec'])}/s" if m.get("networkBytesPerSec") is not None else None,
            "systemStrainPercent": m.get("systemStrainPercent"),
            "backgroundLoadPercent": m.get("backgroundLoadPercent"),
            "foregroundApp": m.get("foregroundProcessName"),
            "processes": m.get("totalProcesses"),
            "needingAttention": m.get("attentionProcesses"),
            "highResource": m.get("highResourceProcesses"),
            "battery": battery,
        },
        f"CPU {round(m['cpuUsagePercent'])}%, memory {round(m['memoryUsagePercent'])}%, status {m.get('systemStatus')}",
    )


class RecommendationArgs(BaseModel):
    areas: list[Literal["process", "startup", "storage"]] = Field(default_factory=list, description="Empty means all.")


def _rec(r: dict[str, Any]) -> dict[str, Any]:
    return {
        "title": r.get("title"),
        "message": shorten(r.get("message"), 220),
        "priority": r.get("priority"),
        "processPid": r.get("processPid"),
        "processName": r.get("processName"),
        "startupItemId": r.get("startupItemId"),
        "storagePath": r.get("storagePath"),
        "metric": r.get("metricHighlight"),
    }


@nyra_tool(
    name="get_recommendations",
    description="ASAO's built-in rule-based recommendations for processes, startup apps and storage.",
    args=RecommendationArgs,
    label=lambda a: "Collecting ASAO's recommendations",
)
async def get_recommendations(a: RecommendationArgs, call: Call) -> Result:
    r = await call.bridge("system.recommendations")
    want = set(a.areas) or {"process", "startup", "storage"}
    data = {
        area: [_rec(x) for x in r.get(f"{area}Recommendations", [])[:10]]
        for area in ("process", "startup", "storage")
        if area in want
    }
    return Result(data, plural(sum(len(v) for v in data.values()), "recommendation"))


TOOLS = [get_processes_list, get_process, end_background_task, get_system_metrics, get_recommendations]
