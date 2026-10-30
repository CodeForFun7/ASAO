import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ActionOutcome, AppSizeUpdate, UninstallFinished, UnusedAppsResult } from "../types/apps";
import type { IgnoreList } from "../types/residual";

export const APPS_EVENTS = {
  SIZE_UPDATE: "APPS_SIZE_UPDATE",
  UNINSTALL_FINISHED: "APPS_UNINSTALL_FINISHED",
} as const;

export async function scanUnusedApps(): Promise<UnusedAppsResult> {
  return invoke<UnusedAppsResult>("apps_scan");
}

export async function getLastUnusedApps(): Promise<UnusedAppsResult | null> {
  return invoke<UnusedAppsResult | null>("apps_get_last");
}

/** Starts the removal the user confirmed. `useFallback` picks the app's second method. */
export async function removeApp(id: string, useFallback: boolean): Promise<ActionOutcome> {
  return invoke<ActionOutcome>("apps_remove", { id, useFallback });
}

export async function ignoreUnusedApp(key: string, name: string): Promise<IgnoreList> {
  return invoke<IgnoreList>("apps_ignore", { key, name });
}

export async function unignoreUnusedApp(key: string): Promise<IgnoreList> {
  return invoke<IgnoreList>("apps_unignore", { key });
}

export async function subscribeToAppEvents(handlers: {
  onSize: (u: AppSizeUpdate) => void;
  onFinished: (f: UninstallFinished) => void;
}): Promise<() => void> {
  const unlisteners: UnlistenFn[] = [];
  try {
    unlisteners.push(await listen<AppSizeUpdate>(APPS_EVENTS.SIZE_UPDATE, (e) => handlers.onSize(e.payload)));
    unlisteners.push(
      await listen<UninstallFinished>(APPS_EVENTS.UNINSTALL_FINISHED, (e) => handlers.onFinished(e.payload))
    );
  } catch {
    // Outside the Tauri runtime (browser preview)
  }
  return () => {
    for (const u of unlisteners) u();
  };
}
