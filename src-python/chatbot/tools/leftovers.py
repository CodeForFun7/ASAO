"""Folders left behind by uninstalled apps."""

import asyncio
from typing import Any, Literal

from pydantic import BaseModel, Field

from ..bridge import BridgeError
from ..formatting import clamp, human_bytes, iso_date, lower, plural
from .base import Call, Result, cached, declined, invalidate, nyra_tool

CONFIDENCE_RANK = {"high": 0, "medium": 1, "low": 2}

CONSEQUENCE = {
    "install-remnant": "Leftover program files from an uninstall. Safe to remove.",
    "cache": "Cache. Safe to remove; rebuilt if the app is reinstalled.",
    "logs": "Logs. Only useful for troubleshooting.",
    "app-data": "Settings and data the app saved. A reinstall would start fresh.",
}


async def leftover_result(call: Call, rescan: bool) -> dict[str, Any]:
    if rescan:
        invalidate("leftovers:")

    async def load() -> dict[str, Any]:
        if not rescan:
            last = await call.bridge("leftovers.last")
            if last:
                return last
        call.progress("Scanning Program Files, AppData and ProgramData for leftovers")
        try:
            return await call.bridge("leftovers.scan")
        except BridgeError as exc:
            if "already" not in lower(str(exc)) and "running" not in lower(str(exc)):
                raise
            for _ in range(120):
                await asyncio.sleep(2)
                last = await call.bridge("leftovers.last")
                if last:
                    return last
            raise

    return await cached("leftovers:result", 120, load)


def folder(c: dict[str, Any], full: bool = False) -> dict[str, Any]:
    out = {
        "path": c["path"],
        "category": c.get("category"),
        "confidence": c.get("confidence"),
        "size": human_bytes(c["sizeBytes"]) if c.get("sizeBytes") is not None else "measuring",
        "needsAdmin": c.get("needsAdmin") or None,
        "inOneDrive": c.get("inOnedrive") or None,
        "lastChanged": iso_date(c.get("newestModifiedMs")),
    }
    if full:
        out.update(
            {
                "reasons": c.get("reasons"),
                "matchedApp": c.get("matchedAppName"),
                "score": c.get("score"),
                "whatRemovingDoes": CONSEQUENCE.get(c.get("category", ""), ""),
            }
        )
    else:
        out["why"] = (c.get("reasons") or [None])[0]
    return out


class LeftoverListArgs(BaseModel):
    rescan: bool = Field(False, description="Rescan instead of using ASAO's last result.")
    min_confidence: Literal["high", "medium", "low"] = Field("medium", description="'low' includes uncertain folders.")
    app_name: str | None = None
    limit: int = 40


@nyra_tool(
    name="get_leftovers_list",
    description="Folders left behind by apps that were uninstalled (program files, caches, logs, app data), grouped by app, with confidence and size. Ignored folders are excluded.",
    args=LeftoverListArgs,
    label=lambda a: "Looking for leftovers of uninstalled apps",
)
async def get_leftovers_list(a: LeftoverListArgs, call: Call) -> Result:
    r = await leftover_result(call, a.rescan)
    limit_rank = CONFIDENCE_RANK[a.min_confidence]
    groups = []
    total_bytes = 0
    folder_count = 0
    for g in r.get("groups", []):
        if a.app_name and lower(a.app_name) not in lower(g.get("appName")):
            continue
        folders = [c for c in g.get("folders", []) if CONFIDENCE_RANK.get(c.get("confidence"), 2) <= limit_rank]
        if not folders:
            continue
        size = sum(c.get("sizeBytes") or 0 for c in folders)
        total_bytes += size
        folder_count += len(folders)
        groups.append(
            {
                "app": g.get("appName"),
                "key": g.get("key"),
                "unknownApp": g.get("isUnknown") or None,
                "confidence": g.get("confidence"),
                "removedOn": iso_date(g.get("removedOnMs")),
                "size": human_bytes(size),
                "folders": [folder(c) for c in folders],
            }
        )
    groups.sort(key=lambda g: (CONFIDENCE_RANK.get(g["confidence"], 2), g["unknownApp"] is True))
    stats = r.get("stats", {})
    data = {
        "scannedAt": iso_date(r.get("finishedMs")),
        "apps": len(groups),
        "folders": folder_count,
        "totalSize": human_bytes(total_bytes),
        "groups": groups[: clamp(a.limit, 40, 1, 200)],
        "emptyHiddenFolders": len(r.get("emptyFolders", [])) or None,
        "foldersChecked": stats.get("foldersChecked"),
        "skippedRecentlyChanged": stats.get("recentSkipped"),
    }
    return Result(data, f"{plural(folder_count, 'leftover folder')} from {plural(len(groups), 'app')}, {human_bytes(total_bytes)}")


class LeftoverDetailArgs(BaseModel):
    path: str | None = Field(None, description="Leftover folder path.")
    app_name: str | None = Field(None, description="All leftovers of this app.")


@nyra_tool(
    name="get_leftover_detail",
    description="Why ASAO believes a folder (or every folder of one app) is a leftover: evidence, confidence, size, last change and what removing it does.",
    args=LeftoverDetailArgs,
    label=lambda a: f"Inspecting leftovers of {a.app_name or a.path}",
)
async def get_leftover_detail(a: LeftoverDetailArgs, call: Call) -> Result:
    r = await leftover_result(call, False)
    found = []
    for g in r.get("groups", []):
        for c in g.get("folders", []):
            if (a.path and lower(c["path"]).rstrip("\\") == lower(a.path).rstrip("\\")) or (a.app_name and lower(a.app_name) in lower(g.get("appName"))):
                found.append({"app": g.get("appName"), "removedOn": iso_date(g.get("removedOnMs")), **folder(c, full=True)})
    if not found:
        return Result({"found": False, "message": "Not in the latest leftover scan."}, "Not found", "error")
    return Result({"folders": found}, f"{plural(len(found), 'folder')} found")


class RemoveLeftoversArgs(BaseModel):
    paths: list[str] = Field(description="Leftover folder paths from get_leftovers_list.")
    permanent: bool = Field(False, description="Default moves them to the Recycle Bin.")


@nyra_tool(
    name="remove_leftovers",
    description="Remove leftover folders found by the leftover scan (re-checked first; folders in use are skipped). Moves to the Recycle Bin by default. The user confirms in the chat first.",
    args=RemoveLeftoversArgs,
    risk="destructive",
    label=lambda a: f"Removing {plural(len(a.paths), 'leftover folder')}",
)
async def remove_leftovers(a: RemoveLeftoversArgs, call: Call) -> Result:
    r = await leftover_result(call, False)
    candidates = {lower(c["path"]).rstrip("\\"): (g, c) for g in r.get("groups", []) for c in g.get("folders", [])}
    chosen, unknown = [], []
    for p in a.paths:
        hit = candidates.get(lower(p).rstrip("\\"))
        (chosen.append(hit) if hit else unknown.append(p))
    if not chosen:
        return Result({"error": "None of these paths are in the latest leftover scan.", "unknown": unknown}, "Unknown paths", "error")
    total = sum(c.get("sizeBytes") or 0 for _, c in chosen)
    needs_admin = any(c.get("needsAdmin") for _, c in chosen)
    message = "Each folder is checked again before removal; folders in use are skipped."
    if any(c.get("category") == "app-data" for _, c in chosen):
        message += " Some contain app settings and data."
    if needs_admin:
        message += " Windows will ask for administrator permission for some folders."
    message += " This cannot be undone." if a.permanent else " You can restore them from the Recycle Bin."
    ok = await call.confirm(
        title=f"{'Permanently delete' if a.permanent else 'Move'} {plural(len(chosen), 'leftover folder')} ({human_bytes(total)}){'' if a.permanent else ' to the Recycle Bin'}?",
        message=message,
        items=[{"label": c["path"], "detail": f"{g.get('appName')} · {c.get('category')} · {human_bytes(c.get('sizeBytes'))}"} for g, c in chosen[:60]],
        total_items=len(chosen),
        confirm_label="Delete permanently" if a.permanent else "Move to Recycle Bin",
        danger="high" if a.permanent else "medium",
    )
    if not ok:
        return declined("removing leftover folders")
    report = await call.bridge("leftovers.clean", {"paths": [c["path"] for _, c in chosen], "permanent": a.permanent})
    invalidate("leftovers:")
    deleted = [o for o in report["outcomes"] if o["status"] == "deleted"]
    other = [{"path": o["path"], "status": o["status"], "reason": o.get("reason")} for o in report["outcomes"] if o["status"] != "deleted"]
    data = {
        "removed": len(deleted),
        "freed": human_bytes(report.get("freedBytes")),
        "toRecycleBin": not a.permanent,
        "notRemoved": other or None,
        "adminDeclined": report.get("adminDeclined") or None,
        "unknownPaths": unknown or None,
    }
    return Result(data, f"Removed {len(deleted)} of {len(chosen)}, freed {data['freed']}", "done" if deleted else "error")


TOOLS = [get_leftovers_list, get_leftover_detail, remove_leftovers]
