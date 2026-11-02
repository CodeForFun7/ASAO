import { useNyraStore } from "../stores/nyra-store";
import { useProcessStore } from "../stores/process-store";
import type { UnusedApp } from "../types/apps";
import { KIND_LABELS as APP_KIND_LABELS, formatAgo } from "../types/apps";
import type { BloatItem } from "../types/bloat";
import { KIND_LABELS as BLOAT_KIND_LABELS, SAFETY_LABELS } from "../types/bloat";
import type { ProcessInfo } from "../types/process";
import { CATEGORY_METADATA } from "../types/process";
import type { ResidualCandidate, ResidualGroup } from "../types/residual";
import { CATEGORY_LABELS as RESIDUAL_CATEGORY_LABELS, CONFIDENCE_LABELS } from "../types/residual";
import type { StartupItem } from "../types/startup";
import type { StorageItem } from "../types/storage";
import { formatStorageBytes, formatStorageTimestampDetailed } from "./storage";
import { formatBytes, formatRate, formatUptime } from "./tauri";

type Field = [label: string, value: string | number | boolean | null | undefined];

const NO_CHANGES = "Only explain it. Do not change, disable, delete or clean anything on my PC.";

function yesNo(value: boolean): string {
  return value ? "yes" : "no";
}

function list(values: string[]): string | null {
  return values.length > 0 ? values.join(", ") : null;
}

/** Builds the message: a plain-language request followed by the entity's known details. */
function buildPrompt(request: string, fields: Field[]): string {
  const lines = fields
    .filter(([, value]) => value !== null && value !== undefined && value !== "")
    .map(([label, value]) => `- ${label}: ${typeof value === "boolean" ? yesNo(value) : value}`);
  return `${request}\n${NO_CHANGES}\n\nDetails from ASAO:\n${lines.join("\n")}`;
}

/** Queues `prompt` as a new Nyra conversation and opens the chat page. */
export function askNyra(prompt: string): void {
  useNyraStore.getState().queuePrompt(prompt);
  useProcessStore.getState().setRoute("assistant");
}

export function startupPrompt(item: StartupItem): string {
  return buildPrompt(
    `Explain the startup item "${item.name}" to me in plain language. Tell me what it is, what it is used for, whether I need it, and what happens if I stop it from starting with Windows.`,
    [
      ["Name", item.name],
      ["Publisher", item.publisher],
      ["Description", item.description],
      ["Startup mechanism", item.startupGroup],
      ["Executable path", item.executablePath],
      ["Command line", item.commandLine],
      ["Registry key", item.registryKey],
      ["Service name", item.serviceName],
      ["Scheduled task", item.taskPath],
      ["Starts with Windows", item.isEnabled],
      ["Running now", item.isCurrentlyRunning],
      ["Boot impact", item.impact],
      ["Boot CPU time", `${item.bootCpuMs} ms`],
      ["Boot disk read", formatBytes(item.bootDiskBytes)],
      ["Memory", formatBytes(item.memoryBytes)],
      ["How often it is used", item.usageFrequency],
      ["ASAO classification", item.classification],
      ["ASAO recommendation", item.recommendation],
      ["If disabled", item.disableConsequences],
    ]
  );
}

export function processPrompt(process: ProcessInfo): string {
  return buildPrompt(
    `Explain the process "${process.name}" (PID ${process.pid}) to me in plain language. Tell me what it is, what it is used for, whether it is normal for it to be running, and whether its resource use looks reasonable.`,
    [
      ["Name", process.name],
      ["Publisher", process.publisher],
      ["Product", process.productName],
      ["Description", process.description],
      ["Category", (CATEGORY_METADATA[process.category] ?? CATEGORY_METADATA.unknown).label],
      ["Executable path", process.executablePath],
      ["Parent PID", process.parentPid],
      ["Status", process.status],
      ["Activity", process.activityState],
      ["CPU", `${process.cpuPercent.toFixed(1)}%`],
      ["Memory", formatBytes(process.memoryBytes)],
      ["Disk", formatRate(process.diskBytesPerSec)],
      ["Threads", process.threadCount],
      ["Running for", formatUptime(process.startedSecondsAgo)],
      ["Starts with Windows", process.isStartup],
      ["System critical", process.isSystemCritical],
      ["Restricted (details hidden by Windows)", process.isRestricted],
    ]
  );
}

export function bloatPrompt(item: BloatItem): string {
  return buildPrompt(
    `Explain "${item.name}" to me in plain language. Tell me what it is, what it is used for, whether I need it, and what would break if I removed it.`,
    [
      ["Name", item.name],
      ["Type", BLOAT_KIND_LABELS[item.kind]],
      ["Publisher", item.publisher],
      ["Category", item.category],
      ["What it does", item.whatItDoes],
      ["What breaks without it", item.whatBreaks],
      ["Install location", item.installLocation],
      ["Size", item.sizeBytes ? formatStorageBytes(item.sizeBytes) : null],
      ["Running now", item.running],
      ["Impact score", `${item.impact.total} of 100`],
      ["Services", list(item.services.map((s) => s.displayName || s.name))],
      ["Startup entries", list(item.startup.map((s) => s.name))],
      ["Scheduled tasks", list(item.tasks.map((t) => t.name))],
      ["Other programs that depend on it", list(item.dependents)],
      ["ASAO safety rating", `${SAFETY_LABELS[item.safety]}. ${item.safetyReason}`],
    ]
  );
}

export function filePrompt(item: StorageItem): string {
  const kind = item.isDir ? "folder" : "file";
  return buildPrompt(
    `Explain this ${kind} to me in plain language: "${item.name}". Tell me what it is, which program or part of Windows it belongs to, and whether it is safe to delete.`,
    [
      ["Name", item.name],
      ["Path", item.path],
      ["Type", item.itemType],
      ["Extension", item.extension],
      ["Size", formatStorageBytes(item.size)],
      ["Modified", formatStorageTimestampDetailed(item.modifiedMs)],
      ["Last accessed", formatStorageTimestampDetailed(item.accessedMs)],
      ["ASAO category", item.category],
      ["ASAO importance", item.importance],
      ["ASAO analysis", item.analysisExplanation],
    ]
  );
}

export function unusedAppPrompt(app: UnusedApp): string {
  return buildPrompt(
    `Explain the app "${app.name}" to me in plain language. Tell me what it is used for and whether it is safe to uninstall if I have not used it in a while.`,
    [
      ["Name", app.name],
      ["Publisher", app.publisher],
      ["Install type", APP_KIND_LABELS[app.kind]],
      ["Install folder", app.installDir],
      ["Size", app.sizeBytes !== null ? formatStorageBytes(app.sizeBytes) : null],
      ["Usage", formatAgo(app.lastUsedMs)],
      ["Installed", app.installedMs ? formatStorageTimestampDetailed(app.installedMs) : null],
      ["Note", app.note],
    ]
  );
}

export function leftoverPrompt(group: ResidualGroup, folder: ResidualCandidate): string {
  return buildPrompt(
    `Explain this leftover folder to me in plain language: "${folder.name}". Tell me what it likely contains, which app it came from, and whether it is safe to delete.`,
    [
      ["Folder", folder.name],
      ["Path", folder.path],
      ["Likely belongs to", folder.matchedAppName ?? (group.isUnknown ? null : group.appName)],
      ["Kind of data", RESIDUAL_CATEGORY_LABELS[folder.category]],
      ["ASAO confidence it is leftover", CONFIDENCE_LABELS[folder.confidence]],
      ["Size", folder.sizeBytes !== null ? formatStorageBytes(folder.sizeBytes) : null],
      ["Last changed", folder.newestModifiedMs ? formatStorageTimestampDetailed(folder.newestModifiedMs) : null],
      ["App removed on", folder.removedOnMs ? formatStorageTimestampDetailed(folder.removedOnMs) : null],
      ["Needs administrator rights", folder.needsAdmin],
      ["Stored in OneDrive", folder.inOnedrive],
      ["Why ASAO flagged it", list(folder.reasons)],
    ]
  );
}
