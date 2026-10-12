import { invoke } from "@tauri-apps/api/core";
import type {
  BootPerformanceSummary,
  StartupItem,
  WprStatus,
} from "../types/startup";

export async function getStartupItems(): Promise<StartupItem[]> {
  return invoke<StartupItem[]>("get_startup_items");
}

export async function getBootPerformanceSummary(): Promise<BootPerformanceSummary> {
  return invoke<BootPerformanceSummary>("get_boot_performance_summary");
}

export async function disableStartupItem(
  itemId: string,
  source: string,
  registryKey?: string | null,
  serviceName?: string | null,
  taskPath?: string | null
): Promise<void> {
  return invoke<void>("disable_startup_item", {
    itemId,
    source,
    registryKey: registryKey ?? null,
    serviceName: serviceName ?? null,
    taskPath: taskPath ?? null,
  });
}

export async function enableStartupItem(
  itemId: string,
  source: string,
  registryKey?: string | null,
  serviceName?: string | null,
  taskPath?: string | null,
  commandLine?: string | null
): Promise<void> {
  return invoke<void>("enable_startup_item", {
    itemId,
    source,
    registryKey: registryKey ?? null,
    serviceName: serviceName ?? null,
    taskPath: taskPath ?? null,
    commandLine: commandLine ?? null,
  });
}

export async function getWprStatus(): Promise<WprStatus> {
  return invoke<WprStatus>("wpr_get_status");
}

export async function startWprBootTrace(): Promise<string> {
  return invoke<string>("wpr_start_boot_trace");
}

export async function cancelWprBootTrace(): Promise<string> {
  return invoke<string>("wpr_cancel_boot_trace");
}
