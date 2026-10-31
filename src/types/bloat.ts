// Mirrors src-tauri/src/bloat (serde camelCase / kebab-case).

export type BloatSafety = "safe" | "usually-safe" | "caution" | "not-recommended" | "protected" | "not-rated";
export type BloatRecommendation = "remove" | "optional" | "keep";
export type BloatCategory =
  | "microsoft-inbox-app"
  | "oem-utility"
  | "trialware"
  | "promoted-game"
  | "promoted-app"
  | "updater-helper"
  | "optional-windows-service";
export type BloatItemKind = "store-app" | "win32-app" | "service" | "startup-entry";
export type ImpactBand = "low" | "medium" | "high";

export interface BloatService {
  name: string;
  displayName: string;
  description: string | null;
  binaryPath: string | null;
  startType: string;
  delayedAutoStart: boolean;
  state: string;
  account: string | null;
  processId: number;
  dependsOn: string[];
  dependents: string[];
  isWindows: boolean;
  plannedStartType: string;
}

export interface BloatStartupEntry {
  id: string;
  name: string;
  source: string;
  command: string | null;
  executable: string | null;
  enabled: boolean;
  registryKey: string | null;
  bootCpuMs: number | null;
  bootDiskBytes: number | null;
}

export interface BloatTask {
  path: string;
  name: string;
  action: string | null;
  enabled: boolean;
  frequent: boolean;
  triggers: string[];
}

export interface ImpactPart {
  points: number;
  max: number;
  value: string;
}

export interface BloatImpact {
  total: number;
  band: ImpactBand;
  startup: ImpactPart;
  cpu: ImpactPart;
  memory: ImpactPart;
  autostart: ImpactPart;
  disk: ImpactPart;
}

export interface BloatItem {
  id: string;
  kind: BloatItemKind;
  name: string;
  publisher: string | null;
  category: BloatCategory | null;
  dbId: string | null;
  whatItDoes: string | null;
  whatBreaks: string | null;
  howToRestore: string | null;
  source: string | null;
  recommendation: BloatRecommendation | null;
  safety: BloatSafety;
  safetyReason: string;
  safetyChecks: string[];
  selectable: boolean;
  preselected: boolean;
  packageFamilyName: string | null;
  packageFullName: string | null;
  packageName: string | null;
  registrySubkey: string | null;
  installLocation: string | null;
  sizeBytes: number | null;
  services: BloatService[];
  startup: BloatStartupEntry[];
  tasks: BloatTask[];
  dependents: string[];
  running: boolean;
  lastUsedMs: number | null;
  impact: BloatImpact;
  plannedChanges: string[];
  undo: string;
  storeUrl: string | null;
}

export interface DeviceContext {
  manufacturer: string | null;
  model: string | null;
  orgManaged: boolean;
  managedReason: string | null;
  gamesInstalled: boolean;
  antivirus: { name: string; enabled: boolean }[];
  defenderPresent: boolean;
  isElevated: boolean;
  startupMeasured: boolean;
}

export interface TweakItem {
  id: string;
  label: string;
  tooltip: string | null;
  category: string;
  groupId: string | null;
  /** Every value in its registry file already matches. Null when it could not be read. */
  applied: boolean | null;
  /** Win11Debloat's default mode selects it (and it is not applied yet). */
  preselected: boolean;
  requiresReboot: boolean;
  needsAdmin: boolean;
  registryFile: string;
}

export interface TweakGroup {
  id: string;
  label: string;
  tooltip: string | null;
  category: string;
  options: { label: string; featureId: string; isDefault: boolean }[];
  /** The option in effect now, if one matches. */
  current: string | null;
}

export interface TweakSet {
  tweaks: TweakItem[];
  groups: TweakGroup[];
  notOffered: [string, string][];
}

export interface AppPreset {
  name: string;
  appIds: string[];
}

export interface BloatScanResult {
  scanId: number;
  startedMs: number;
  finishedMs: number;
  items: BloatItem[];
  device: DeviceContext;
  stats: { apps: number; services: number; startupEntries: number; tasks: number; sampleSeconds: number };
  dbVersion: number;
  dbUpdated: string;
  dbSource: string;
  tweaks: TweakSet;
  presets: AppPreset[];
  w11dSource: string;
  log: string[];
}

export interface BloatScanProgress {
  phase: "inventory" | "sampling" | "scoring";
  message: string;
  percent: number | null;
}

export interface BloatInfo {
  devMode: boolean;
  dataDir: string;
  dbOverridePath: string;
  dbOverrideActive: boolean;
  dbVersion: number;
  dbUpdated: string;
  dbEntries: number;
  dbProblem: string | null;
  dryRunDefault: boolean;
  dryRunToggleVisible: boolean;
  dryRunForced: boolean;
  actionLog: string;
  dryRunLog: string;
  historyFile: string;
}

export const SAFETY_LABELS: Record<BloatSafety, string> = {
  safe: "Safe to remove",
  "usually-safe": "Usually safe",
  caution: "Caution",
  "not-recommended": "Not recommended",
  protected: "Protected",
  "not-rated": "Not rated",
};

export const SAFETY_HELP: Record<BloatSafety, string> = {
  safe: "ASAO's list says it can go, nothing depends on it, and it is easy to reinstall",
  "usually-safe": "Nothing in Windows needs it, but some people use it",
  caution: "Something may rely on it, so ASAO will not change it",
  "not-recommended": "Windows, your hardware or your security needs it",
  protected: "Windows does not allow removing it",
  "not-rated": "Not in ASAO's list, so ASAO makes no recommendation and will not change it",
};

export const CATEGORY_LABELS: Record<BloatCategory, string> = {
  "microsoft-inbox-app": "Microsoft app",
  "oem-utility": "PC maker utility",
  trialware: "Trialware",
  "promoted-game": "Promoted game",
  "promoted-app": "Promoted app",
  "updater-helper": "Updater",
  "optional-windows-service": "Windows service",
};

export const KIND_LABELS: Record<BloatItemKind, string> = {
  "store-app": "Store app",
  "win32-app": "Desktop app",
  service: "Service",
  "startup-entry": "Startup entry",
};

export const IMPACT_LABELS: Record<ImpactBand, string> = {
  high: "High impact",
  medium: "Medium impact",
  low: "Low impact",
};

// ---- Changes (stage 2) ----

export type ActionKind =
  | { type: "restore-point" }
  | { type: "disable-startup"; name: string; source: string; registryKey: string | null }
  | { type: "disable-task"; path: string }
  | { type: "configure-service"; name: string; displayName: string; startType: string; stop: boolean }
  | { type: "uninstall-store"; packageFullName: string; packageName: string }
  | { type: "remove-provisioned"; packageName: string }
  | { type: "uninstall-win32"; displayName: string; registrySubkey: string | null }
  | { type: "apply-tweak"; featureId: string; label: string; registryFile: string };

export interface BloatAction {
  id: string;
  itemId: string;
  itemName: string;
  kind: ActionKind;
  description: string;
  needsAdmin: boolean;
  undoable: boolean;
}

export interface BloatPlan {
  actions: BloatAction[];
  rejected: { itemId: string; name: string; reason: string }[];
  needsAdmin: boolean;
  freedBytes: number;
  servicesStopped: number;
  notUndoable: string[];
}

export type OutcomeStatus = "done" | "would-do" | "skipped" | "failed" | "protected" | "not-confirmed";

export interface BloatOutcome {
  action: BloatAction;
  status: OutcomeStatus;
  message: string | null;
  original: unknown;
}

export type RestorePointStatus = "created" | "recent" | "unavailable" | "skipped";

export interface BloatApplyReport {
  batchId: string;
  dryRun: boolean;
  restorePoint: { status: RestorePointStatus; message: string };
  outcomes: BloatOutcome[];
  rejected: { itemId: string; name: string; reason: string }[];
  stopped: boolean;
  adminRequested: boolean;
  adminDeclined: boolean;
  logPath: string;
  uninstalled: string[];
  /** A Windows setting changed; some show only after File Explorer restarts. */
  settingsChanged: boolean;
  /** A changed setting needs a restart. */
  restartNeeded: boolean;
  error: string | null;
}

export interface BloatApplyProgress {
  done: number;
  total: number;
  current: string;
}

export interface HistoryAction {
  outcome: BloatOutcome;
  undoneMs: number | null;
  undoHint: string;
  canUndo: boolean;
  storeUrl: string | null;
}

export interface HistoryBatch {
  id: string;
  timeMs: number;
  restorePoint: { status: RestorePointStatus; message: string };
  actions: HistoryAction[];
}

export interface UndoResult {
  ok: boolean;
  message: string;
}

/** The group an action appears under in the confirm dialog. */
export function actionGroup(kind: ActionKind): string {
  switch (kind.type) {
    case "restore-point":
      return "Safety net";
    case "disable-startup":
      return "Disable startup entry";
    case "disable-task":
      return "Disable scheduled task";
    case "configure-service":
      return "Disable service";
    case "uninstall-store":
    case "uninstall-win32":
      return "Uninstall";
    case "remove-provisioned":
      return "Remove for new users";
    case "apply-tweak":
      return "Change Windows setting";
  }
}
