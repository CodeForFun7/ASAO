"""Drives, folders, file search, duplicates, temporary files and deletion."""

from typing import Any, Literal

from pydantic import BaseModel, Field

from ..formatting import DAY_MS, MB, clamp, human_bytes, iso_date, now_ms, plural
from .base import Call, Result, cached, declined, gather_limited, invalidate, nyra_tool

FileKind = Literal[
    "image", "video", "audio", "document", "archive", "disk-image", "executable", "code", "temporary", "font", "system", "other"
]


def entry(e: dict[str, Any]) -> dict[str, Any]:
    """A file or folder from the Rust side, trimmed for the model."""
    return {
        "path": e["path"],
        "size": human_bytes(e.get("size")),
        "kind": e.get("kind"),
        "folder": True if e.get("isDir") else None,
        "files": e.get("itemCount"),
        "modified": iso_date(e.get("modifiedMs")),
        "accessed": iso_date(e.get("accessedMs")),
        "hidden": True if e.get("hidden") else None,
    }


def storage_item(i: dict[str, Any]) -> dict[str, Any]:
    return {
        "name": i["name"],
        "path": i["path"],
        "folder": True if i.get("isDir") else None,
        "size": human_bytes(i.get("size")),
        "type": i.get("itemType"),
        "category": i.get("category"),
        "importance": i.get("importance"),
        "modified": iso_date(i.get("modifiedMs")),
    }


# ---------------------------------------------------------------------------
# Drives and drive-wide overview (uses ASAO's storage scan index)
# ---------------------------------------------------------------------------


class NoArgs(BaseModel):
    pass


@nyra_tool(
    name="get_user_folders",
    description="Real paths of the user's Desktop, Documents, Downloads, Pictures, Music, Videos, AppData, Temp, OneDrive and Program Files. Use this to resolve folder names the user mentions.",
    args=NoArgs,
    label=lambda a: "Finding your user folders",
)
async def get_user_folders(a: NoArgs, call: Call) -> Result:
    folders = await cached("fs:known", 300, lambda: call.bridge("fs.known_folders"))
    return Result({f["key"]: f["path"] for f in folders if f.get("exists")}, f"{len(folders)} folders located")


@nyra_tool(
    name="get_drives_list",
    description="All drives with file system, type, capacity, used and free space.",
    args=NoArgs,
    label=lambda a: "Listing drives",
)
async def get_drives_list(a: NoArgs, call: Call) -> Result:
    drives = await call.bridge("storage.drives")
    rows = [
        {
            "drive": d["drive"],
            "name": d.get("name"),
            "fileSystem": d.get("fileSystem"),
            "type": d.get("driveType"),
            "total": human_bytes(d.get("totalCapacity")),
            "used": human_bytes(d.get("usedCapacity")),
            "free": human_bytes(d.get("freeCapacity")),
            "usedPercent": round(d.get("usagePercentage", 0)),
        }
        for d in drives
    ]
    return Result({"drives": rows}, ", ".join(f"{r['drive']} {r['free']} free" for r in rows))


async def drive_snapshot(call: Call, drive: str | None, refresh: bool) -> dict[str, Any]:
    """Latest scan of a drive, scanning it first when there is none (or on refresh)."""
    key = f"storage:snapshot:{(drive or 'default').upper()}"
    if refresh:
        invalidate(key)

    async def load() -> dict[str, Any]:
        if not refresh:
            snap = await call.bridge("storage.snapshot", {"drive": drive})
            if snap:
                return snap
        call.progress(f"Scanning {drive or 'the system drive'} (this can take a minute)")
        return await call.bridge("storage.scan", {"drive": drive})

    return await cached(key, 600, load)


class OverviewArgs(BaseModel):
    drive: str | None = Field(None, description="Drive letter such as 'C:'. Default: system drive.")
    refresh: bool = Field(False, description="Rescan even if a previous scan exists.")


@nyra_tool(
    name="get_storage_overview",
    description="Drive-wide storage analysis: space by category (system, apps, user files, cache, temporary), insights with reclaimable space, largest folders and files. Scans the drive if it has not been scanned.",
    args=OverviewArgs,
    label=lambda a: f"Analysing storage on {a.drive or 'the system drive'}",
)
async def get_storage_overview(a: OverviewArgs, call: Call) -> Result:
    s = await drive_snapshot(call, a.drive, a.refresh)
    drive = next((d for d in s.get("drives", []) if d["drive"].upper() == s["selectedDrive"].rstrip("\\").upper()), None)
    data = {
        "drive": s["selectedDrive"],
        "scannedAt": iso_date(s.get("scanTimestampMs")),
        "capacity": {"total": human_bytes(drive["totalCapacity"]), "free": human_bytes(drive["freeCapacity"]), "usedPercent": round(drive["usagePercentage"])} if drive else None,
        "filesAnalyzed": s.get("filesAnalyzed"),
        "byCategory": [{"category": c["label"], "size": human_bytes(c["bytes"]), "percent": round(c["percentage"], 1), "items": c["itemCount"]} for c in s.get("distribution", [])],
        "insights": [
            {
                "title": i["title"],
                "description": i["description"],
                "size": human_bytes(i.get("size")),
                "action": i.get("action"),
                "targetPath": i.get("targetPath"),
                "examples": [x["path"] for x in i.get("affectedItems", [])[:5]],
            }
            for i in s.get("insights", [])
        ],
        "largestFolders": [storage_item(i) for i in s.get("largestFolders", [])[:12]],
        "largestFiles": [storage_item(i) for i in s.get("largestFiles", [])[:12]],
    }
    return Result(data, f"{s['selectedDrive']}: {len(data['insights'])} insights, {data['capacity']['free'] if drive else '?'} free")


# ---------------------------------------------------------------------------
# Folders and files
# ---------------------------------------------------------------------------


class FolderContentsArgs(BaseModel):
    path: str = Field(description="Full folder path, e.g. C:\\Users\\Me\\Downloads.")
    sort_by: Literal["size", "name", "modified"] = "size"
    limit: int = 60


@nyra_tool(
    name="get_folder_contents",
    description="List the files and subfolders directly inside one folder, with size, type, storage category and importance.",
    args=FolderContentsArgs,
    label=lambda a: f"Opening {a.path}",
)
async def get_folder_contents(a: FolderContentsArgs, call: Call) -> Result:
    listing = await call.bridge("storage.list_dir", {"path": a.path})
    if listing.get("errorMessage"):
        reason = "Access denied" if listing.get("permissionDenied") else listing["errorMessage"]
        return Result({"path": listing.get("path"), "error": reason}, reason, "error")
    items = listing.get("items", [])
    if a.sort_by == "name":
        items = sorted(items, key=lambda i: i["name"].lower())
    elif a.sort_by == "modified":
        items = sorted(items, key=lambda i: i.get("modifiedMs") or 0, reverse=True)
    total = sum(i.get("size", 0) for i in items)
    folders = sum(1 for i in items if i.get("isDir"))
    return Result(
        {
            "path": listing["path"],
            "parent": listing.get("parentPath"),
            "folders": folders,
            "files": len(items) - folders,
            "totalSize": human_bytes(total),
            "items": [storage_item(i) for i in items[: clamp(a.limit, 60, 1, 300)]],
        },
        f"{plural(len(items) - folders, 'file')}, {plural(folders, 'folder')}, {human_bytes(total)}",
    )


class PathArgs(BaseModel):
    path: str = Field(description="Full path.")


@nyra_tool(
    name="get_folder_details",
    description="Deep analysis of a folder and everything inside it: total size, file and folder counts, breakdown by file kind and extension, largest files, largest subfolders, oldest/newest changes and files untouched for a year.",
    args=PathArgs,
    label=lambda a: f"Analysing {a.path}",
)
async def get_folder_details(a: PathArgs, call: Call) -> Result:
    s = await call.bridge("fs.summary", {"path": a.path, "top": 15})
    data = {
        "path": s["path"],
        "totalSize": human_bytes(s["totalSize"]),
        "files": s["fileCount"],
        "folders": s["folderCount"],
        "byKind": [{"kind": k["key"], "files": k["count"], "size": human_bytes(k["bytes"])} for k in s["byKind"]],
        "topExtensions": [{"ext": k["key"], "files": k["count"], "size": human_bytes(k["bytes"])} for k in s["topExtensions"][:15]],
        "largestFiles": [entry(e) for e in s["largestFiles"]],
        "largestChildren": [
            {"name": c["name"], "path": c["path"], "folder": c["isDir"] or None, "size": human_bytes(c["size"]), "files": c["fileCount"]}
            for c in s["children"][:20]
        ],
        "directChildren": s["childCount"],
        "newestChange": iso_date(s.get("newestModifiedMs")),
        "oldestChange": iso_date(s.get("oldestModifiedMs")),
        "untouchedForAYear": {"files": s["staleFiles"], "size": human_bytes(s["staleBytes"])},
        "accessDeniedFolders": s["accessDenied"] or None,
        "incomplete": s["truncated"] or None,
    }
    return Result(data, f"{human_bytes(s['totalSize'])} in {plural(s['fileCount'], 'file')}")


@nyra_tool(
    name="get_file_details",
    description="Details of one file or folder: size (folders measured recursively), dates, attributes, storage category and importance, ASAO's explanation, and whether Nyra is allowed to delete it.",
    args=PathArgs,
    label=lambda a: f"Inspecting {a.path}",
)
async def get_file_details(a: PathArgs, call: Call) -> Result:
    d = await call.bridge("fs.details", {"path": a.path})
    if not d.get("exists"):
        return Result({"path": d["path"], "exists": False}, "Does not exist", "error")
    data = {
        "path": d["path"],
        "folder": d["isDir"] or None,
        "size": human_bytes(d["size"]),
        "files": d.get("fileCount"),
        "folders": d.get("folderCount"),
        "sizeIncomplete": (not d.get("sizeComplete")) or None,
        "kind": d.get("kind"),
        "type": d.get("itemType"),
        "category": d.get("category"),
        "importance": d.get("importance"),
        "explanation": d.get("explanation"),
        "created": iso_date(d.get("createdMs")),
        "modified": iso_date(d.get("modifiedMs")),
        "accessed": iso_date(d.get("accessedMs")),
        "readonly": d.get("readonly") or None,
        "hidden": d.get("hidden") or None,
        "system": d.get("system") or None,
        "deletable": d.get("protectedReason") is None,
        "protectedReason": d.get("protectedReason"),
    }
    return Result(data, f"{data['size']} · {d.get('category') or d.get('kind')}")


class SearchArgs(BaseModel):
    root: str = Field(description="Full folder path to search in (searched recursively). Use a drive root like 'D:\\' to search a whole drive.")
    name_terms: list[str] = Field(
        default_factory=list,
        description="Match ANY of these in file/folder names (case-insensitive; spaces, '-', '_' and '.' are ignored). For 'related to X' requests include variants, people, titles and abbreviations.",
    )
    match_full_path: bool = Field(False, description="Also match name_terms against the whole path (files inside a folder named after the term).")
    extensions: list[str] = Field(default_factory=list, description="e.g. ['jpg','png'] without dots.")
    kinds: list[FileKind] = Field(default_factory=list)
    min_size_mb: float | None = None
    max_size_mb: float | None = None
    modified_older_than_days: int | None = None
    modified_within_days: int | None = None
    not_accessed_for_days: int | None = None
    include_folders: bool = Field(False, description="Return folders whose names match name_terms as single entries (with their total size) instead of listing their files.")
    max_depth: int | None = Field(None, description="1 = only the folder itself.")
    sort_by: Literal["size", "newest", "oldest", "name", "least-accessed"] = "size"
    limit: int = 200


def _search_request(a: SearchArgs) -> dict[str, Any]:
    now = now_ms()
    return {
        "root": a.root,
        "nameTerms": a.name_terms,
        "matchFullPath": a.match_full_path,
        "extensions": a.extensions,
        "kinds": list(a.kinds),
        "minSize": int(a.min_size_mb * MB) if a.min_size_mb is not None else None,
        "maxSize": int(a.max_size_mb * MB) if a.max_size_mb is not None else None,
        "modifiedBeforeMs": now - a.modified_older_than_days * DAY_MS if a.modified_older_than_days else None,
        "modifiedAfterMs": now - a.modified_within_days * DAY_MS if a.modified_within_days else None,
        "accessedBeforeMs": now - a.not_accessed_for_days * DAY_MS if a.not_accessed_for_days else None,
        "includeFolders": a.include_folders,
        "maxDepth": a.max_depth,
        "limit": clamp(a.limit, 200, 1, 1000),
        "sortBy": a.sort_by,
    }


def _search_label(a: SearchArgs) -> str:
    what = []
    if a.name_terms:
        what.append("“" + "”, “".join(a.name_terms[:4]) + "”" + ("…" if len(a.name_terms) > 4 else ""))
    if a.kinds:
        what.append("/".join(a.kinds))
    if a.extensions:
        what.append(", ".join("." + e.lstrip(".") for e in a.extensions[:5]))
    if a.min_size_mb:
        what.append(f"over {a.min_size_mb:g} MB")
    return f"Searching {a.root} for {' · '.join(what) or 'files'}"


async def run_search(call: Call, request: dict[str, Any]) -> Result:
    r = await call.bridge("fs.search", request)
    data = {
        "root": r["root"],
        "totalMatches": r["totalMatches"],
        "totalSize": human_bytes(r["totalBytes"]),
        "shown": len(r["matches"]),
        "matches": [entry(e) for e in r["matches"]],
        "itemsChecked": r["scannedEntries"],
        "accessDeniedFolders": r["accessDenied"] or None,
        "incomplete": "The search hit its time or size limit; narrow the folder for complete results." if r["truncated"] else None,
    }
    return Result(data, f"{plural(r['totalMatches'], 'match', 'matches')} ({human_bytes(r['totalBytes'])}) in {r['root']}")


@nyra_tool(
    name="search_files",
    description="Search a folder tree for files (and optionally folders) by name terms, extensions, file kinds, size and dates. Matches names and paths, not file contents or image contents.",
    args=SearchArgs,
    label=_search_label,
)
async def search_files(a: SearchArgs, call: Call) -> Result:
    return await run_search(call, _search_request(a))


class LargeFilesArgs(BaseModel):
    path: str | None = Field(None, description="Folder to search. Omit to use the drive scan.")
    drive: str | None = None
    min_size_mb: float = 100
    limit: int = 25


@nyra_tool(
    name="get_large_files",
    description="Largest files in a folder (searched recursively) or on a whole drive.",
    args=LargeFilesArgs,
    label=lambda a: f"Finding files over {a.min_size_mb:g} MB in {a.path or a.drive or 'the system drive'}",
)
async def get_large_files(a: LargeFilesArgs, call: Call) -> Result:
    limit = clamp(a.limit, 25, 1, 200)
    if a.path:
        return await run_search(call, {"root": a.path, "minSize": int(a.min_size_mb * MB), "sortBy": "size", "limit": limit})
    s = await drive_snapshot(call, a.drive, False)
    files = [f for f in s.get("largestFiles", []) if f.get("size", 0) >= a.min_size_mb * MB][:limit]
    return Result(
        {"drive": s["selectedDrive"], "scannedAt": iso_date(s.get("scanTimestampMs")), "files": [storage_item(f) for f in files]},
        f"{plural(len(files), 'large file')} on {s['selectedDrive']}",
    )


class LargeFoldersArgs(BaseModel):
    path: str | None = Field(None, description="Folder whose subfolders are compared. Omit to use the drive scan.")
    drive: str | None = None
    limit: int = 20


@nyra_tool(
    name="get_large_folders",
    description="Largest folders on a drive, or the largest subfolders inside one folder.",
    args=LargeFoldersArgs,
    label=lambda a: f"Finding the largest folders in {a.path or a.drive or 'the system drive'}",
)
async def get_large_folders(a: LargeFoldersArgs, call: Call) -> Result:
    limit = clamp(a.limit, 20, 1, 100)
    if a.path:
        s = await call.bridge("fs.summary", {"path": a.path, "top": 1})
        rows = [{"path": c["path"], "size": human_bytes(c["size"]), "files": c["fileCount"]} for c in s["children"] if c["isDir"]][:limit]
        return Result({"path": s["path"], "totalSize": human_bytes(s["totalSize"]), "folders": rows}, f"{len(rows)} subfolders compared")
    snap = await drive_snapshot(call, a.drive, False)
    rows = [storage_item(f) for f in snap.get("largestFolders", [])[:limit]]
    return Result({"drive": snap["selectedDrive"], "folders": rows}, f"Top {len(rows)} folders on {snap['selectedDrive']}")


class DuplicateArgs(BaseModel):
    root: str = Field(description="Folder to check (recursively).")
    min_size_kb: int = Field(1024, description="Ignore smaller files (default 1 MB).")
    extensions: list[str] = Field(default_factory=list)
    kinds: list[FileKind] = Field(default_factory=list)
    max_groups: int = 30


@nyra_tool(
    name="get_duplicate_files",
    description="Find files with identical contents (same size and SHA-256) in a folder tree. In each group the first file is the oldest, usually the original.",
    args=DuplicateArgs,
    label=lambda a: f"Looking for duplicate files in {a.root}",
)
async def get_duplicate_files(a: DuplicateArgs, call: Call) -> Result:
    r = await call.bridge(
        "fs.duplicates",
        {"root": a.root, "minSize": a.min_size_kb * 1024, "extensions": a.extensions, "kinds": list(a.kinds), "maxGroups": clamp(a.max_groups, 30, 1, 200)},
    )
    groups = [
        {"size": human_bytes(g["size"]), "copies": g["count"], "wasted": human_bytes(g["wastedBytes"]), "keep": g["files"][0]["path"], "duplicates": [f["path"] for f in g["files"][1:]]}
        for g in r["groups"]
    ]
    data = {
        "root": r["root"],
        "duplicateGroups": r["totalGroups"],
        "extraCopies": r["duplicateFiles"],
        "reclaimable": human_bytes(r["reclaimableBytes"]),
        "groups": groups,
        "filesCompared": r["filesScanned"],
        "incomplete": "Stopped at the time limit; results cover part of the folder." if r["truncated"] else None,
    }
    return Result(data, f"{plural(r['totalGroups'], 'duplicate group')}, {human_bytes(r['reclaimableBytes'])} reclaimable")


@nyra_tool(
    name="get_temporary_files",
    description="Temporary files, caches (Windows, browsers, developer tools), crash dumps, Windows Update downloads and the Recycle Bin, with sizes and whether they are safe to clean. Ids are used by clean_temporary_files.",
    args=NoArgs,
    label=lambda a: "Measuring temporary files and caches",
)
async def get_temporary_files(a: NoArgs, call: Call) -> Result:
    locations = await cached("fs:temp", 60, lambda: call.bridge("fs.temp"))
    rows = [
        {"id": l["id"], "label": l["label"], "size": human_bytes(l["sizeBytes"]), "files": l["fileCount"], "safety": l["safety"], "needsAdmin": l["needsAdmin"] or None, "description": l["description"]}
        for l in locations
    ]
    safe = sum(l["sizeBytes"] for l in locations if l["safety"] == "safe")
    total = sum(l["sizeBytes"] for l in locations)
    return Result({"total": human_bytes(total), "safeToClean": human_bytes(safe), "locations": rows}, f"{human_bytes(total)} found, {human_bytes(safe)} safe to clean")


class CleanTempArgs(BaseModel):
    location_ids: list[str] = Field(description="Ids from get_temporary_files. 'recycle-bin' empties the Recycle Bin.")
    min_age_hours: int = Field(24, description="Keep files changed more recently than this.")


@nyra_tool(
    name="clean_temporary_files",
    description="Permanently delete the contents of chosen temporary/cache locations (files in use or changed recently are skipped). The user confirms in the chat first.",
    args=CleanTempArgs,
    risk="destructive",
    label=lambda a: f"Cleaning {plural(len(a.location_ids), 'temporary location')}",
)
async def clean_temporary_files(a: CleanTempArgs, call: Call) -> Result:
    locations = await cached("fs:temp", 60, lambda: call.bridge("fs.temp"))
    by_id = {l["id"]: l for l in locations}
    chosen = [by_id[i] for i in a.location_ids if i in by_id]
    unknown = [i for i in a.location_ids if i not in by_id]
    if not chosen:
        return Result({"error": "None of these ids exist. Call get_temporary_files first.", "unknownIds": unknown}, "Unknown locations", "error")
    total = sum(l["sizeBytes"] for l in chosen)
    ok = await call.confirm(
        title=f"Permanently delete about {human_bytes(total)} of temporary files?",
        message=f"Files changed in the last {a.min_age_hours} hours and files in use are kept. This cannot be undone, but apps recreate caches as needed.",
        items=[{"label": l["label"], "detail": f"{human_bytes(l['sizeBytes'])} · {plural(l['fileCount'], 'file')}"} for l in chosen],
        confirm_label="Clean",
        danger="high" if any(l["id"] == "recycle-bin" for l in chosen) else "medium",
    )
    if not ok:
        return declined("cleaning temporary files")

    report: dict[str, Any] = {"freedBytes": 0, "deletedFiles": 0, "skippedInUse": 0, "skippedRecent": 0, "locations": []}
    folder_ids = [l["id"] for l in chosen if l["id"] != "recycle-bin"]
    if folder_ids:
        report = await call.bridge("fs.clean_temp", {"locationIds": folder_ids, "minAgeHours": a.min_age_hours})
    if any(l["id"] == "recycle-bin" for l in chosen):
        call.progress("Emptying the Recycle Bin")
        freed = (await call.bridge("fs.empty_recycle_bin")).get("freedBytes", 0)
        report["freedBytes"] += freed
        report["locations"].append({"label": "Recycle Bin", "freedBytes": freed})
    invalidate("fs:temp")
    data = {
        "freed": human_bytes(report["freedBytes"]),
        "deletedFiles": report["deletedFiles"],
        "keptInUse": report["skippedInUse"],
        "keptRecent": report["skippedRecent"],
        "perLocation": [{"label": l["label"], "freed": human_bytes(l["freedBytes"])} for l in report["locations"]],
        "unknownIds": unknown or None,
    }
    return Result(data, f"Freed {data['freed']}")


@nyra_tool(
    name="empty_recycle_bin",
    description="Permanently delete everything in the Recycle Bin. The user confirms in the chat first.",
    args=NoArgs,
    risk="destructive",
    label=lambda a: "Emptying the Recycle Bin",
)
async def empty_recycle_bin(a: NoArgs, call: Call) -> Result:
    locations = await call.bridge("fs.temp")
    bin_ = next((l for l in locations if l["id"] == "recycle-bin"), None)
    if not bin_ or bin_["fileCount"] == 0:
        return Result({"freed": "0 B", "message": "The Recycle Bin is already empty."}, "Already empty")
    ok = await call.confirm(
        title="Empty the Recycle Bin?",
        message=f"{plural(bin_['fileCount'], 'item')} ({human_bytes(bin_['sizeBytes'])}) will be deleted for good.",
        confirm_label="Empty Recycle Bin",
        danger="high",
    )
    if not ok:
        return declined("emptying the Recycle Bin")
    freed = (await call.bridge("fs.empty_recycle_bin")).get("freedBytes", 0)
    invalidate("fs:temp")
    return Result({"freed": human_bytes(freed)}, f"Freed {human_bytes(freed)}")


class CategoriesArgs(BaseModel):
    path: str | None = Field(None, description="Folder to break down by file kind.")
    drive: str | None = Field(None, description="Drive to break down by storage category (used when path is omitted).")


@nyra_tool(
    name="get_file_categories",
    description="What kinds of data take space: by file kind (images, videos, documents, ...) for a folder, or by storage category (system, applications, user files, cache, temporary) for a drive.",
    args=CategoriesArgs,
    label=lambda a: f"Breaking down {a.path or a.drive or 'the system drive'} by file type",
)
async def get_file_categories(a: CategoriesArgs, call: Call) -> Result:
    if a.path:
        s = await call.bridge("fs.summary", {"path": a.path, "top": 1})
        kinds = [{"kind": k["key"], "files": k["count"], "size": human_bytes(k["bytes"]), "percent": round(100 * k["bytes"] / s["totalSize"], 1) if s["totalSize"] else 0} for k in s["byKind"]]
        return Result({"path": s["path"], "totalSize": human_bytes(s["totalSize"]), "byKind": kinds, "topExtensions": [{"ext": k["key"], "size": human_bytes(k["bytes"])} for k in s["topExtensions"][:10]]}, f"{len(kinds)} kinds in {human_bytes(s['totalSize'])}")
    snap = await drive_snapshot(call, a.drive, False)
    cats = [{"category": c["label"], "size": human_bytes(c["bytes"]), "percent": round(c["percentage"], 1), "items": c["itemCount"]} for c in snap.get("distribution", [])]
    return Result({"drive": snap["selectedDrive"], "byCategory": cats}, f"{len(cats)} categories on {snap['selectedDrive']}")


class UnusedFilesArgs(BaseModel):
    root: str = Field(description="Folder to check.")
    not_used_for_days: int = 365
    min_size_mb: float = 1
    limit: int = 50


@nyra_tool(
    name="get_unused_files",
    description="Files not opened or changed for a long time in a folder tree, oldest first. Windows does not always record when a file was opened, so the newer of last-opened and last-changed is used.",
    args=UnusedFilesArgs,
    label=lambda a: f"Finding files unused for {a.not_used_for_days} days in {a.root}",
)
async def get_unused_files(a: UnusedFilesArgs, call: Call) -> Result:
    return await run_search(
        call,
        {
            "root": a.root,
            "accessedBeforeMs": now_ms() - a.not_used_for_days * DAY_MS,
            "minSize": int(a.min_size_mb * MB),
            "sortBy": "least-accessed",
            "limit": clamp(a.limit, 50, 1, 500),
        },
    )


# ---------------------------------------------------------------------------
# Deletion
# ---------------------------------------------------------------------------


class RemoveArgs(BaseModel):
    paths: list[str] = Field(description="Exact full paths, as returned by other tools.")
    permanent: bool = Field(False, description="True deletes for good. Default moves to the Recycle Bin. Only use true when the user asked for permanent deletion.")
    reason: str = Field("", description="What these are and why they are being removed, in one sentence. Shown to the user.")


async def remove_paths(a: RemoveArgs, call: Call, want_dirs: bool) -> Result:
    paths = list(dict.fromkeys(p.strip() for p in a.paths if p and p.strip()))
    if not paths:
        return Result({"error": "No paths given."}, "No paths", "error")
    if len(paths) > 5000:
        return Result({"error": "Too many paths at once (limit 5000)."}, "Too many paths", "error")

    call.progress(f"Checking {plural(len(paths), 'path')}")
    details = await gather_limited(paths[:400], 16, lambda p: _details(call, p))
    checked = {d["input"]: d for d in details}
    ok_paths: list[str] = []
    rejected: list[dict[str, str]] = []
    total = 0
    for p in paths:
        d = checked.get(p)
        if d is None:  # beyond the detail limit: Rust still checks protection
            ok_paths.append(p)
            continue
        if d.get("error"):
            rejected.append({"path": p, "reason": d["error"]})
        elif not d.get("exists"):
            rejected.append({"path": p, "reason": "Does not exist"})
        elif d.get("protectedReason"):
            rejected.append({"path": p, "reason": "Protected: " + d["protectedReason"]})
        elif bool(d.get("isDir")) != want_dirs:
            rejected.append({"path": p, "reason": "Is a folder; use remove_folder" if d.get("isDir") else "Is a file; use remove_file"})
        else:
            ok_paths.append(d["path"])
            total += d.get("size", 0)
    if not ok_paths:
        return Result({"deleted": 0, "rejected": rejected}, "Nothing could be removed", "error")

    noun = ("folder", "folders") if want_dirs else ("file", "files")
    count = plural(len(ok_paths), *noun)
    where = "Permanently delete" if a.permanent else "Move"
    title = f"{where} {count} ({human_bytes(total)}){'' if a.permanent else ' to the Recycle Bin'}?"
    message = a.reason or ""
    if a.permanent:
        message += " This cannot be undone."
    else:
        message += " You can restore them from the Recycle Bin."
    if rejected:
        message += f" {plural(len(rejected), 'path')} will be skipped (protected or missing)."
    items = []
    for p in ok_paths[:60]:
        d = checked.get(p) or next((x for x in details if x.get("path") == p), None)
        items.append({"label": p, "detail": human_bytes(d.get("size")) if d else ""})
    big = total > 10 * 1024**3 or len(ok_paths) > 100
    ok = await call.confirm(
        title=title,
        message=message.strip(),
        items=items,
        total_items=len(ok_paths),
        confirm_label="Delete permanently" if a.permanent else "Move to Recycle Bin",
        danger="high" if a.permanent or big or want_dirs else "medium",
    )
    if not ok:
        return declined(f"removing {count}")

    report = await call.bridge("fs.delete", {"paths": ok_paths, "permanent": a.permanent})
    invalidate("fs:temp")
    invalidate("storage:")
    failed = [{"path": o["path"], "reason": o.get("reason")} for o in report["outcomes"] if o["status"] != "deleted"]
    data = {
        "removed": report["deleted"],
        "freed": human_bytes(report["freedBytes"]),
        "toRecycleBin": not a.permanent,
        "notRemoved": failed + rejected,
    }
    summary = f"Removed {report['deleted']} of {len(ok_paths)}, {human_bytes(report['freedBytes'])}"
    if failed:
        summary += f", {len(failed)} failed"
    return Result(data, summary, "done" if report["deleted"] else "error")


async def _details(call: Call, path: str) -> dict[str, Any]:
    try:
        d = await call.bridge("fs.details", {"path": path})
        return {"input": path, **d}
    except Exception as exc:  # reported per path
        return {"input": path, "error": str(exc)}


@nyra_tool(
    name="remove_file",
    description="Delete one or more files (to the Recycle Bin by default). System and protected locations are refused. The user sees the full list and confirms in the chat first.",
    args=RemoveArgs,
    risk="destructive",
    label=lambda a: f"Removing {plural(len(a.paths), 'file')}",
)
async def remove_file(a: RemoveArgs, call: Call) -> Result:
    return await remove_paths(a, call, want_dirs=False)


@nyra_tool(
    name="remove_folder",
    description="Delete one or more folders with everything inside (to the Recycle Bin by default). System and protected folders are refused. The user confirms in the chat first.",
    args=RemoveArgs,
    risk="destructive",
    label=lambda a: f"Removing {plural(len(a.paths), 'folder')}",
)
async def remove_folder(a: RemoveArgs, call: Call) -> Result:
    return await remove_paths(a, call, want_dirs=True)


@nyra_tool(
    name="open_in_explorer",
    description="Open a folder, or show a file, in Windows File Explorer for the user.",
    args=PathArgs,
    label=lambda a: f"Opening {a.path} in File Explorer",
)
async def open_in_explorer(a: PathArgs, call: Call) -> Result:
    await call.bridge("storage.open", {"path": a.path})
    return Result({"opened": a.path}, "Opened in File Explorer")


READ_TOOLS = [
    get_user_folders, get_drives_list, get_storage_overview, get_folder_contents, get_folder_details,
    search_files, get_file_details, get_large_files, get_large_folders, get_duplicate_files,
    get_temporary_files, get_file_categories, get_unused_files, open_in_explorer,
]
DELETE_TOOLS = [remove_file, remove_folder]
TEMP_TOOLS = [get_temporary_files, clean_temporary_files, empty_recycle_bin]
TOOLS = READ_TOOLS + DELETE_TOOLS + [clean_temporary_files, empty_recycle_bin]
