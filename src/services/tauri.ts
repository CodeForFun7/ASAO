import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ProcessInfo,
  ProcessSnapshotPayload,
  SystemMetrics,
} from "../types/process";

export const TAURI_EVENTS = {
  PROCESS_SNAPSHOT: "PROCESS_SNAPSHOT",
  SYSTEM_METRICS: "SYSTEM_METRICS",
  PROCESS_ERROR: "PROCESS_ERROR",
} as const;

export async function getProcesses(): Promise<ProcessInfo[]> {
  return invoke<ProcessInfo[]>("get_processes");
}

export async function getSystemMetrics(): Promise<SystemMetrics> {
  return invoke<SystemMetrics>("get_system_metrics");
}

export async function getFullSnapshot(): Promise<ProcessSnapshotPayload> {
  return invoke<ProcessSnapshotPayload>("get_full_snapshot");
}

export async function startMonitoring(): Promise<void> {
  return invoke<void>("start_monitoring");
}

export async function stopMonitoring(): Promise<void> {
  return invoke<void>("stop_monitoring");
}

export async function minimizeWindow(): Promise<void> {
  return invoke<void>("window_minimize");
}

export async function toggleMaximizeWindow(): Promise<boolean> {
  return invoke<boolean>("window_toggle_maximize");
}

export async function closeWindow(): Promise<void> {
  return invoke<void>("window_close");
}

export interface TelemetrySubscriptionHandlers {
  onProcessSnapshot: (processes: ProcessInfo[]) => void;
  onSystemMetrics: (metrics: SystemMetrics) => void;
  onError: (errorMessage: string) => void;
}

export async function subscribeToTelemetry(
  handlers: TelemetrySubscriptionHandlers
): Promise<() => void> {
  const unlisteners: UnlistenFn[] = [];

  try {
    const unlistenProcesses = await listen<ProcessInfo[]>(
      TAURI_EVENTS.PROCESS_SNAPSHOT,
      (event) => {
        handlers.onProcessSnapshot(event.payload);
      }
    );
    unlisteners.push(unlistenProcesses);

    const unlistenMetrics = await listen<SystemMetrics>(
      TAURI_EVENTS.SYSTEM_METRICS,
      (event) => {
        handlers.onSystemMetrics(event.payload);
      }
    );
    unlisteners.push(unlistenMetrics);

    const unlistenError = await listen<string>(
      TAURI_EVENTS.PROCESS_ERROR,
      (event) => {
        handlers.onError(event.payload);
      }
    );
    unlisteners.push(unlistenError);
  } catch (err) {
    handlers.onError(
      err instanceof Error
        ? err.message
        : "Unable to bind Tauri telemetry event listeners."
    );
  }

  return () => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  };
}

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 MB";
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) {
    return `${(mb / 1024).toFixed(1)} GB`;
  }
  if (mb >= 100) {
    return `${Math.round(mb)} MB`;
  }
  if (mb >= 1) {
    return `${mb.toFixed(1)} MB`;
  }
  const kb = bytes / 1024;
  return `${Math.max(1, Math.round(kb))} KB`;
}

export function formatRate(bytesPerSec: number): string {
  if (bytesPerSec <= 512) return "0 KB/s";
  const mb = bytesPerSec / (1024 * 1024);
  if (mb >= 1) {
    return `${mb.toFixed(1)} MB/s`;
  }
  const kb = bytesPerSec / 1024;
  return `${Math.max(1, Math.round(kb))} KB/s`;
}

export function formatUptime(startedSecondsAgo: number | null): string {
  if (startedSecondsAgo === null || startedSecondsAgo < 0) {
    return "Restricted";
  }
  if (startedSecondsAgo < 60) {
    return `${startedSecondsAgo}s ago`;
  }
  const minutes = Math.floor(startedSecondsAgo / 60);
  if (minutes < 60) {
    return `${minutes}m ago`;
  }
  const hours = Math.floor(minutes / 60);
  const remMinutes = minutes % 60;
  if (hours < 24) {
    return `${hours}h ${remMinutes}m ago`;
  }
  const days = Math.floor(hours / 24);
  const remHours = hours % 24;
  return `${days}d ${remHours}h ago`;
}
