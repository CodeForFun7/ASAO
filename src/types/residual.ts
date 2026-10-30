// Mirrors src-tauri/src/residual (serde camelCase / kebab-case).

export type ResidualRootKind =
  | "program-files"
  | "program-files-x86"
  | "roaming-app-data"
  | "local-app-data"
  | "local-app-data-low"
  | "program-data";

export type ResidualConfidence = "high" | "medium" | "low";

export type ResidualCategory = "install-remnant" | "cache" | "logs" | "app-data";

export interface ResidualCandidate {
  id: string;
  path: string;
  name: string;
  rootKind: ResidualRootKind;
  depth: number;
  category: ResidualCategory;
  confidence: ResidualConfidence;
  score: number;
  reasons: string[];
  matchedAppKey: string | null;
  matchedAppName: string | null;
  removedOnMs: number | null;
  newestModifiedMs: number | null;
  sizeBytes: number | null;
  needsAdmin: boolean;
  inOnedrive: boolean;
  preselected: boolean;
}

export interface ResidualGroup {
  key: string;
  appName: string;
  isUnknown: boolean;
  confidence: ResidualConfidence;
  removedOnMs: number | null;
  folders: ResidualCandidate[];
}

export interface ResidualScanStats {
  rootsScanned: number;
  foldersChecked: number;
  ownedSkipped: number;
  excludedSkipped: number;
  recentSkipped: number;
  accessDenied: number;
  installedApps: number;
  removedAppsKnown: number;
  referencesAvailable: boolean;
}

export interface ResidualScanResult {
  scanId: number;
  startedMs: number;
  finishedMs: number;
  minAgeDays: number;
  groups: ResidualGroup[];
  stats: ResidualScanStats;
  log: string[];
}

export interface ResidualScanProgress {
  phase: "inventory" | "matching";
  currentPath: string;
  foldersChecked: number;
  candidatesFound: number;
}

export interface ResidualSizeUpdate {
  scanId: number;
  path: string;
  sizeBytes: number;
  newestModifiedMs: number | null;
}

export interface ResidualSizingDone {
  scanId: number;
  totalBytes: number;
}

export type CleanStatus = "deleted" | "would-delete" | "skipped" | "failed";

export interface CleanOutcome {
  path: string;
  name: string;
  category: ResidualCategory | null;
  sizeBytes: number | null;
  status: CleanStatus;
  reason: string | null;
  recycled: boolean;
  needsAdmin: boolean;
  inOnedrive: boolean;
}

export interface CleanReport {
  dryRun: boolean;
  permanent: boolean;
  freedBytes: number;
  outcomes: CleanOutcome[];
  adminRequested: boolean;
  adminDeclined: boolean;
  logPath: string;
}

export interface IgnoredFolder {
  path: string;
  addedMs: number;
}

export interface IgnoredApp {
  key: string;
  name: string;
  addedMs: number;
}

export interface IgnoreList {
  folders: IgnoredFolder[];
  apps: IgnoredApp[];
}

export interface ResidualInfo {
  devMode: boolean;
  isElevated: boolean;
  dataDir: string;
  deletionLog: string;
  dryRunLog: string;
  scanLog: string;
}

export const CATEGORY_LABELS: Record<ResidualCategory, string> = {
  "install-remnant": "Install remnant",
  cache: "Cache",
  logs: "Logs",
  "app-data": "App data",
};

export const CONFIDENCE_LABELS: Record<ResidualConfidence, string> = {
  high: "High",
  medium: "Medium",
  low: "Uncertain",
};

/** Plain-language consequence shown in the confirm dialog, per category. */
export function categoryConsequence(category: ResidualCategory, appName: string | null): string {
  switch (category) {
    case "install-remnant":
      return "Leftover program files from an uninstall. Safe to remove.";
    case "cache":
      return "Safe to remove. Rebuilt automatically if you reinstall.";
    case "logs":
      return "Safe to remove. Only useful for troubleshooting.";
    case "app-data":
      return `Settings and data ${appName ?? "the app"} saved on this PC. If you reinstall it, it will start fresh.`;
  }
}
