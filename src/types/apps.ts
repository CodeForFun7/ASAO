// Mirrors src-tauri/src/apps (serde camelCase / kebab-case).

export type AppKind = "registered" | "store" | "unregistered" | "steam" | "epic";

export type RemovalMethod =
  | "uninstaller"
  | "msi-product"
  | "windows-settings"
  | "store-package"
  | "remove-folder"
  | "steam-uninstall"
  | "open-epic";

export type UsageSource = "launch-record" | "prefetch" | "asao-monitor";

export interface UnusedApp {
  id: string;
  key: string;
  name: string;
  publisher: string | null;
  kind: AppKind;
  method: RemovalMethod;
  fallback: RemovalMethod | null;
  note: string | null;
  lastUsedMs: number | null;
  lastUsedSource: UsageSource | null;
  installedMs: number | null;
  sizeBytes: number | null;
  installDir: string | null;
  needsAdmin: boolean;
}

export interface UnusedAppsResult {
  scanId: number;
  apps: UnusedApp[];
  prefetchAvailable: boolean;
  trackingSinceMs: number | null;
  considered: number;
  runningSkipped: number;
}

export type ActionStatus = "launched" | "done" | "declined" | "failed";

export interface ActionOutcome {
  id: string;
  status: ActionStatus;
  message: string;
  watching: boolean;
}

export interface AppSizeUpdate {
  scanId: number;
  id: string;
  sizeBytes: number;
}

export interface UninstallFinished {
  id: string;
  name: string;
  removed: boolean;
}

export const KIND_LABELS: Record<AppKind, string> = {
  registered: "Installed",
  store: "Store app",
  unregistered: "Not registered",
  steam: "Steam",
  epic: "Epic Games",
};

export const METHOD_LABELS: Record<RemovalMethod, string> = {
  uninstaller: "Uninstall",
  "msi-product": "Uninstall",
  "windows-settings": "Open Windows Settings",
  "store-package": "Remove",
  "remove-folder": "Remove program folder",
  "steam-uninstall": "Uninstall in Steam",
  "open-epic": "Open Epic Games",
};

export const USAGE_SOURCE_LABELS: Record<UsageSource, string> = {
  "launch-record": "Windows' record of programs you launched",
  prefetch: "Windows Prefetch",
  "asao-monitor": "ASAO saw it running",
};

/** What happens when the user confirms, in plain words. */
export function methodExplanation(method: RemovalMethod, name: string): string {
  switch (method) {
    case "uninstaller":
      return `${name}'s own uninstaller opens. Follow its steps. ASAO notices when the app is gone.`;
    case "msi-product":
      return `Windows Installer removes ${name}. Windows may ask for administrator permission.`;
    case "windows-settings":
      return `Windows Settings opens on the Apps list. Find ${name} there and choose Uninstall.`;
    case "store-package":
      return `${name} is removed for your account, the same as choosing Uninstall in the Start menu.`;
    case "remove-folder":
      return `${name} was never registered with Windows, so it has no uninstaller. Its program folder and its shortcuts go to the Recycle Bin, where you can restore them.`;
    case "steam-uninstall":
      return `Steam opens and asks to uninstall ${name}. Confirm it there.`;
    case "open-epic":
      return `Epic Games Launcher opens. Uninstall ${name} from your library.`;
  }
}

/** "5 months ago" style text. */
export function formatAgo(ms: number | null, now = Date.now()): string {
  if (!ms) return "No record of use";
  const days = Math.max(0, Math.floor((now - ms) / 86_400_000));
  if (days === 0) return "Used today";
  if (days < 30) return `Last used ${days} ${days === 1 ? "day" : "days"} ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `Last used ${months} ${months === 1 ? "month" : "months"} ago`;
  const years = Math.floor(days / 365);
  return `Last used ${years} ${years === 1 ? "year" : "years"} ago`;
}
