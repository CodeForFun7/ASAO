import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BloatApplyProgress,
  BloatApplyReport,
  BloatInfo,
  BloatPlan,
  BloatScanProgress,
  BloatScanResult,
  HistoryBatch,
  UndoResult,
} from "../types/bloat";

export const BLOAT_EVENTS = {
  SCAN_PROGRESS: "BLOAT_SCAN_PROGRESS",
  APPLY_PROGRESS: "BLOAT_APPLY_PROGRESS",
} as const;

/** Read-only scan: inventory, about 30 s of CPU sampling, then scoring. */
export async function scanBloat(): Promise<BloatScanResult> {
  return invoke<BloatScanResult>("bloat_scan");
}

export async function cancelBloatScan(): Promise<void> {
  return invoke<void>("bloat_cancel_scan");
}

export async function getLastBloatResult(): Promise<BloatScanResult | null> {
  return invoke<BloatScanResult | null>("bloat_get_last_result");
}

export async function getBloatInfo(): Promise<BloatInfo> {
  return invoke<BloatInfo>("bloat_get_info");
}

export async function subscribeToBloatProgress(onProgress: (p: BloatScanProgress) => void): Promise<UnlistenFn> {
  return listen<BloatScanProgress>(BLOAT_EVENTS.SCAN_PROGRESS, (e) => onProgress(e.payload));
}

/** What the ticked items would change. Read-only. */
export async function planBloat(
  selectedIds: string[],
  selectedTweaks: string[],
  removeProvisioned: boolean
): Promise<BloatPlan> {
  return invoke<BloatPlan>("bloat_plan", { selectedIds, selectedTweaks, removeProvisioned });
}

export async function applyBloat(
  selectedIds: string[],
  selectedTweaks: string[],
  dryRun: boolean,
  removeProvisioned: boolean,
  continueWithoutRestorePoint: boolean
): Promise<BloatApplyReport> {
  return invoke<BloatApplyReport>("bloat_apply", {
    selectedIds,
    selectedTweaks,
    dryRun,
    removeProvisioned,
    continueWithoutRestorePoint,
  });
}

/** Restarts File Explorer so taskbar, Start and File Explorer settings show at once. */
export async function restartExplorer(): Promise<void> {
  return invoke<void>("bloat_restart_explorer");
}

export async function getBloatHistory(): Promise<{ batches: HistoryBatch[] }> {
  return invoke<{ batches: HistoryBatch[] }>("bloat_history");
}

export async function undoBloatChange(batchId: string, actionId: string): Promise<UndoResult> {
  return invoke<UndoResult>("bloat_undo", { batchId, actionId });
}

export async function subscribeToApplyProgress(onProgress: (p: BloatApplyProgress) => void): Promise<UnlistenFn> {
  return listen<BloatApplyProgress>(BLOAT_EVENTS.APPLY_PROGRESS, (e) => onProgress(e.payload));
}
