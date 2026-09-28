import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  StorageAnalysisSnapshot,
  StorageDirectoryListing,
  StorageDrive,
  StorageItem,
  StorageScanProgress,
} from "../types/storage";

export const STORAGE_EVENTS = {
  SCAN_PROGRESS: "STORAGE_SCAN_PROGRESS",
  SCAN_COMPLETE: "STORAGE_SCAN_COMPLETE",
} as const;

export async function getStorageDrives(): Promise<StorageDrive[]> {
  return invoke<StorageDrive[]>("get_storage_drives");
}

export async function getStorageSnapshot(
  drive?: string
): Promise<StorageAnalysisSnapshot | null> {
  return invoke<StorageAnalysisSnapshot | null>("get_storage_snapshot", {
    drive: drive ?? null,
  });
}

export async function startStorageScan(
  drive?: string
): Promise<StorageAnalysisSnapshot> {
  return invoke<StorageAnalysisSnapshot>("start_storage_scan", {
    drive: drive ?? null,
  });
}

export async function cancelStorageScan(): Promise<void> {
  return invoke<void>("cancel_storage_scan");
}

export async function readStorageDirectory(
  path: string,
  drive?: string
): Promise<StorageDirectoryListing> {
  return invoke<StorageDirectoryListing>("read_storage_directory", {
    path,
    drive: drive ?? null,
  });
}

export async function searchStorageItems(
  query: string,
  drive?: string,
  currentPath?: string
): Promise<StorageItem[]> {
  return invoke<StorageItem[]>("search_storage_items", {
    query,
    drive: drive ?? null,
    currentPath: currentPath ?? null,
  });
}

export async function openStorageLocation(path: string): Promise<void> {
  return invoke<void>("open_storage_location", { path });
}

export async function subscribeToStorageScanProgress(
  onProgress: (progress: StorageScanProgress) => void
): Promise<UnlistenFn> {
  return listen<StorageScanProgress>(STORAGE_EVENTS.SCAN_PROGRESS, (event) => {
    onProgress(event.payload);
  });
}

export function formatStorageBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || Number.isNaN(bytes)) {
    return "Unavailable";
  }
  if (bytes <= 0) return "0 B";

  const tb = bytes / (1024 * 1024 * 1024 * 1024);
  if (tb >= 1) {
    return `${tb.toFixed(2)} TB`;
  }

  const gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 100) {
    return `${Math.round(gb)} GB`;
  }
  if (gb >= 10) {
    return `${gb.toFixed(1)} GB`;
  }
  if (gb >= 1) {
    return `${gb.toFixed(2)} GB`;
  }

  const mb = bytes / (1024 * 1024);
  if (mb >= 100) {
    return `${Math.round(mb)} MB`;
  }
  if (mb >= 1) {
    return `${mb.toFixed(1)} MB`;
  }

  const kb = bytes / 1024;
  if (kb >= 1) {
    return `${Math.round(kb)} KB`;
  }

  return `${bytes} B`;
}

export function formatDriveCapacityPair(
  usedBytes: number,
  totalBytes: number
): string {
  const tbTotal = totalBytes / (1024 * 1024 * 1024 * 1024);
  if (tbTotal >= 1) {
    const usedTb = (usedBytes / (1024 * 1024 * 1024 * 1024)).toFixed(2);
    const totalTb = tbTotal.toFixed(2);
    return `${usedTb} / ${totalTb} TB`;
  }

  const usedGb = Math.round(usedBytes / (1024 * 1024 * 1024));
  const totalGb = Math.round(totalBytes / (1024 * 1024 * 1024));
  return `${usedGb} / ${totalGb} GB`;
}

export function formatStorageTimestamp(
  timestampMs: number | null | undefined
): string {
  if (!timestampMs || timestampMs <= 0) {
    return "Unavailable";
  }
  try {
    const date = new Date(timestampMs);
    if (Number.isNaN(date.getTime())) return "Unavailable";
    return date.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "2-digit",
    });
  } catch {
    return "Unavailable";
  }
}

export function formatStorageTimestampDetailed(
  timestampMs: number | null | undefined
): string {
  if (!timestampMs || timestampMs <= 0) {
    return "Unavailable";
  }
  try {
    const date = new Date(timestampMs);
    if (Number.isNaN(date.getTime())) return "Unavailable";
    return date.toLocaleString(undefined, {
      year: "numeric",
      month: "short",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return "Unavailable";
  }
}

export interface BreadcrumbSegment {
  label: string;
  path: string;
}

export function parsePathBreadcrumbs(fullPath: string): BreadcrumbSegment[] {
  if (!fullPath) return [];
  const normalized = fullPath.replace(/\//g, "\\");
  const parts = normalized.split("\\").filter((p) => p.length > 0);
  if (parts.length === 0) return [];

  const segments: BreadcrumbSegment[] = [];
  let accum = "";

  for (let i = 0; i < parts.length; i++) {
    const part = parts[i];
    if (i === 0 && part.endsWith(":")) {
      accum = `${part}\\`;
      segments.push({
        label: `${part}\\`,
        path: accum,
      });
    } else {
      accum = accum.endsWith("\\") ? `${accum}${part}` : `${accum}\\${part}`;
      segments.push({
        label: part,
        path: accum,
      });
    }
  }

  return segments;
}
