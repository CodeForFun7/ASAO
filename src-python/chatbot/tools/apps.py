"""Unused and rarely used applications."""

import asyncio
from typing import Any

from pydantic import BaseModel, Field

from ..bridge import BridgeError
from ..formatting import clamp, days_since, human_bytes, iso_date, lower, plural
from .base import Call, Result, cached, declined, invalidate, nyra_tool

METHOD_LABELS = {
    "uninstaller": "App's own uninstaller",
    "msi-product": "Windows Installer",
    "windows-settings": "Windows Settings (manual)",
    "store-package": "Remove Store app",
    "remove-folder": "Program folder to the Recycle Bin",
    "steam-uninstall": "Uninstall in Steam",
    "open-epic": "Uninstall in Epic Games Launcher",
}


def method_explanation(method: str, name: str) -> str:
    return {
        "uninstaller": f"{name}'s own uninstaller opens. Follow its steps; ASAO notices when the app is gone.",
        "msi-product": f"Windows Installer removes {name}. Windows may ask for administrator permission.",
        "windows-settings": f"Windows Settings opens on the Apps list. Find {name} there and choose Uninstall.",
        "store-package": f"{name} is removed for your account, like choosing Uninstall in the Start menu.",
        "remove-folder": f"{name} was never registered with Windows, so its program folder and shortcuts go to the Recycle Bin.",
        "steam-uninstall": f"Steam opens and asks to uninstall {name}.",
        "open-epic": f"Epic Games Launcher opens. Uninstall {name} from your library.",
    }.get(method, f"ASAO starts the removal of {name}.")


async def unused_apps(call: Call, rescan: bool) -> dict[str, Any]:
    if rescan:
        invalidate("apps:")

    async def load() -> dict[str, Any]:
        if not rescan:
            last = await call.bridge("apps.last")
            if last:
                return last
        call.progress("Checking installed apps and when each was last used")
        try:
            return await call.bridge("apps.scan")
        except BridgeError as exc:
            if "already running" not in str(exc):
                raise
            # ASAO's own Unused apps view is scanning; wait for its result.
            for _ in range(90):
                await asyncio.sleep(2)
                last = await call.bridge("apps.last")
                if last:
                    return last
            raise

    return await cached("apps:result", 300, load)


def compact(app: dict[str, Any]) -> dict[str, Any]:
    return {
        "id": app["id"],
        "name": app["name"],
        "publisher": app.get("publisher"),
        "kind": app.get("kind"),
        "lastUsed": iso_date(app.get("lastUsedMs")) or "no record of use",
        "daysSinceUse": days_since(app.get("lastUsedMs")),
        "installed": iso_date(app.get("installedMs")),
        "size": human_bytes(app["sizeBytes"]) if app.get("sizeBytes") else None,
        "removal": METHOD_LABELS.get(app.get("method"), app.get("method")),
        "needsAdmin": app.get("needsAdmin") or None,
    }


class UnusedListArgs(BaseModel):
    rescan: bool = Field(False, description="Rescan instead of using ASAO's last result.")
    name_contains: str | None = None
    min_days_unused: int | None = None
    limit: int = 40


@nyra_tool(
    name="get_unused_app_list",
    description="Installed apps (desktop, Store, Steam, Epic and unregistered) that have not been used for a long time, with last use, install date, size and how each can be removed.",
    args=UnusedListArgs,
    label=lambda a: "Finding apps you have not used",
)
async def get_unused_app_list(a: UnusedListArgs, call: Call) -> Result:
    r = await unused_apps(call, a.rescan)
    apps = r.get("apps", [])
    if a.name_contains:
        apps = [x for x in apps if lower(a.name_contains) in lower(x["name"])]
    if a.min_days_unused:
        apps = [x for x in apps if (days_since(x.get("lastUsedMs")) or 10**6) >= a.min_days_unused]
    apps = sorted(apps, key=lambda x: (x.get("lastUsedMs") or 0, -(x.get("sizeBytes") or 0)))
    known = sum(x.get("sizeBytes") or 0 for x in apps)
    data = {
        "count": len(apps),
        "totalKnownSize": human_bytes(known),
        "usageTrackedSince": iso_date(r.get("trackingSinceMs")),
        "prefetchAvailable": r.get("prefetchAvailable"),
        "appsConsidered": r.get("considered"),
        "apps": [compact(x) for x in apps[: clamp(a.limit, 40, 1, 200)]],
    }
    return Result(data, f"{plural(len(apps), 'unused app')}, {human_bytes(known)}")


class AppDetailArgs(BaseModel):
    id: str | None = Field(None, description="App id from get_unused_app_list.")
    name: str | None = None


def find_app(apps: list[dict[str, Any]], app_id: str | None, name: str | None) -> dict[str, Any] | None:
    for x in apps:
        if app_id and x["id"] == app_id:
            return x
    if name:
        matches = [x for x in apps if lower(name) in lower(x["name"])]
        if matches:
            return matches[0]
    return None


@nyra_tool(
    name="get_unused_apps_detail",
    description="Full details of one unused app: install folder and size, publisher, how ASAO knows when it was last used, and exactly what removing it does.",
    args=AppDetailArgs,
    label=lambda a: f"Inspecting app {a.name or a.id}",
)
async def get_unused_apps_detail(a: AppDetailArgs, call: Call) -> Result:
    r = await unused_apps(call, False)
    app = find_app(r.get("apps", []), a.id, a.name)
    if not app:
        return Result({"found": False, "message": "Not in the unused apps list (it may be in use, ignored, or not installed)."}, "Not found", "error")
    data = {
        **compact(app),
        "installDir": app.get("installDir"),
        "lastUsedSource": app.get("lastUsedSource"),
        "note": app.get("note"),
        "whatRemovalDoes": method_explanation(app.get("method", ""), app["name"]),
        "alternativeRemoval": METHOD_LABELS.get(app.get("fallback")) if app.get("fallback") else None,
    }
    if app.get("installDir") and not app.get("sizeBytes"):
        try:
            d = await call.bridge("fs.details", {"path": app["installDir"]})
            data["size"] = human_bytes(d.get("size"))
        except BridgeError:
            pass
    return Result(data, f"{app['name']}: last used {data['lastUsed']}")


class UninstallArgs(BaseModel):
    id: str = Field(description="App id from get_unused_app_list.")
    use_alternative: bool = Field(False, description="Use the app's alternative removal method.")


@nyra_tool(
    name="uninstall_app",
    description="Start removing an app from the unused apps list (its uninstaller, Windows Installer, Store removal, ...). The user confirms in the chat first; some methods open a window the user must finish.",
    args=UninstallArgs,
    risk="destructive",
    label=lambda a: f"Removing app {a.id}",
)
async def uninstall_app(a: UninstallArgs, call: Call) -> Result:
    r = await unused_apps(call, False)
    app = find_app(r.get("apps", []), a.id, None)
    if not app:
        return Result({"error": "Unknown app id. Call get_unused_app_list first."}, "Not found", "error")
    method = app.get("fallback") if a.use_alternative and app.get("fallback") else app.get("method", "")
    ok = await call.confirm(
        title=f"Remove {app['name']}?",
        message=method_explanation(method, app["name"]),
        items=[{"label": app["name"], "detail": " · ".join(x for x in [app.get("publisher"), compact(app)["size"], f"last used {compact(app)['lastUsed']}"] if x)}],
        confirm_label="Remove app",
        danger="high",
    )
    if not ok:
        return declined(f"removing {app['name']}")
    outcome = await call.bridge("apps.remove", {"id": app["id"], "useFallback": bool(a.use_alternative and app.get("fallback"))})
    invalidate("apps:")
    status = outcome.get("status")
    return Result(
        {"status": status, "message": outcome.get("message"), "asaoIsWatchingForRemoval": outcome.get("watching")},
        {"done": "Removed", "launched": "Uninstaller opened", "declined": "Admin permission declined", "failed": "Failed"}.get(status, status or "?"),
        "error" if status in ("failed", "declined") else "done",
    )


TOOLS = [get_unused_app_list, get_unused_apps_detail, uninstall_app]
