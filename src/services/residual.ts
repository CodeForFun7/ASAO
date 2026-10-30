import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  CleanReport,
  IgnoreList,
  ResidualInfo,
  ResidualScanProgress,
  ResidualScanResult,
  ResidualSizeUpdate,
  ResidualSizingDone,
} from "../types/residual";

export const RESIDUAL_EVENTS = {
  SCAN_PROGRESS: "RESIDUAL_SCAN_PROGRESS",
  SIZE_UPDATE: "RESIDUAL_SIZE_UPDATE",
  SIZING_DONE: "RESIDUAL_SIZING_DONE",
} as const;

export async function scanLeftovers(): Promise<ResidualScanResult> {
  return invoke<ResidualScanResult>("residual_scan");
}

export async function cancelLeftoverScan(): Promise<void> {
  return invoke<void>("residual_cancel_scan");
}

export async function getLastLeftoverResult(): Promise<ResidualScanResult | null> {
  return invoke<ResidualScanResult | null>("residual_get_last_result");
}

/** Deletes (or, in dry run, only checks and logs) the given folders from the latest scan. */
export async function cleanLeftovers(
  paths: string[],
  dryRun: boolean,
  permanent: boolean
): Promise<CleanReport> {
  return invoke<CleanReport>("residual_clean", { paths, dryRun, permanent });
}

/** Moves the hidden empty folders of the latest scan to the Recycle Bin (or, in dry run, only logs them). */
export async function cleanEmptyLeftovers(dryRun: boolean): Promise<CleanReport> {
  return invoke<CleanReport>("residual_clean_empty", { dryRun });
}

export async function getIgnoreList(): Promise<IgnoreList> {
  return invoke<IgnoreList>("residual_get_ignore_list");
}

export async function ignoreFolder(path: string): Promise<IgnoreList> {
  return invoke<IgnoreList>("residual_ignore_folder", { path });
}

export async function ignoreApp(key: string, name: string): Promise<IgnoreList> {
  return invoke<IgnoreList>("residual_ignore_app", { key, name });
}

export async function unignoreFolder(path: string): Promise<IgnoreList> {
  return invoke<IgnoreList>("residual_unignore_folder", { path });
}

export async function unignoreApp(key: string): Promise<IgnoreList> {
  return invoke<IgnoreList>("residual_unignore_app", { key });
}

export async function openRecycleBin(): Promise<void> {
  return invoke<void>("residual_open_recycle_bin");
}

export async function getResidualInfo(): Promise<ResidualInfo> {
  return invoke<ResidualInfo>("residual_get_info");
}

export async function subscribeToLeftoverEvents(handlers: {
  onProgress: (p: ResidualScanProgress) => void;
  onSize: (u: ResidualSizeUpdate) => void;
  onSizingDone: (d: ResidualSizingDone) => void;
}): Promise<() => void> {
  const unlisteners: UnlistenFn[] = [];
  try {
    unlisteners.push(
      await listen<ResidualScanProgress>(RESIDUAL_EVENTS.SCAN_PROGRESS, (e) => handlers.onProgress(e.payload))
    );
    unlisteners.push(
      await listen<ResidualSizeUpdate>(RESIDUAL_EVENTS.SIZE_UPDATE, (e) => handlers.onSize(e.payload))
    );
    unlisteners.push(
      await listen<ResidualSizingDone>(RESIDUAL_EVENTS.SIZING_DONE, (e) => handlers.onSizingDone(e.payload))
    );
  } catch {
    // Outside the Tauri runtime (browser preview)
  }
  return () => {
    for (const u of unlisteners) u();
  };
}
