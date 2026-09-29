export type StartupSource =
  | "registry-run-user"
  | "registry-run-machine"
  | "registry-run-once-user"
  | "registry-run-once-machine"
  | "startup-folder-user"
  | "startup-folder-machine"
  | "scheduled-task"
  | "windows-service"
  | "winlogon-shell"
  | "winlogon-userinit"
  | "winlogon-notify"
  | "app-init"
  | "boot-execute"
  | "policy-run";

export type StartupGroup =
  | "Registry Startup"
  | "Startup Folder"
  | "Scheduled Tasks"
  | "Windows Services"
  | "Winlogon / System Startup"
  | "Other Autostart Mechanisms";

export type StartupImpact = "low" | "medium" | "high";

export type UsageFreq = "frequently" | "occasionally" | "rarely";

export type ItemClass = "essential" | "optional" | "user-dependent";

export type Recommendation = "keep" | "investigate" | "disable";

export interface StartupItem {
  id: string;
  name: string;
  executablePath: string | null;
  commandLine: string | null;
  source: StartupSource;
  startupGroup: StartupGroup;
  publisher: string | null;
  description: string | null;
  isCurrentlyRunning: boolean;
  pid: number | null;
  isEnabled: boolean;
  bootCpuMs: number;
  bootDiskBytes: number;
  memoryBytes: number;
  bootDurationMs: number;
  impact: StartupImpact;
  usageFrequency: UsageFreq;
  classification: ItemClass;
  recommendation: Recommendation;
  disableMethod: string;
  disableConsequences: string;
  restoreMethod: string;
  registryKey: string | null;
  serviceName: string | null;
  taskPath: string | null;
  bootTimelinePhase: string | null;
  etwTraced: boolean;
}

export interface WprStatus {
  isAvailable: boolean;
  isRecording: boolean;
  isBootConfigured: boolean;
  message: string;
}

export interface BootPhaseMetric {
  name: string;
  durationMs: number;
  cpuMs: number;
  diskBytes: number;
  description: string;
}

export interface BootPerformanceSummary {
  lastBootTime: string;
  uptimeSeconds: number;
  totalStartupItems: number;
  highImpactCount: number;
  estimatedBootDelayMs: number;
  totalBootCpuMs: number;
  totalBootDiskBytes: number;
  totalMemoryImpactBytes: number;
  etwStatus: string;
  bootPhases: BootPhaseMetric[];
}

export const STARTUP_GROUP_ORDER: StartupGroup[] = [
  "Registry Startup",
  "Startup Folder",
  "Scheduled Tasks",
  "Windows Services",
  "Winlogon / System Startup",
  "Other Autostart Mechanisms",
];

export const STARTUP_GROUP_DESCRIPTIONS: Record<StartupGroup, string> = {
  "Registry Startup":
    "Applications registered in HKCU and HKLM Run/RunOnce keys that automatically execute at user logon",
  "Startup Folder":
    "Shortcuts and executables placed in user and all-users Start Menu Startup directories",
  "Scheduled Tasks":
    "Windows Task Scheduler items configured to trigger upon system boot or user logon",
  "Windows Services":
    "Background system services configured with Automatic or Delayed-Automatic start mode",
  "Winlogon / System Startup":
    "Core Windows interactive logon shell components and authentication providers",
  "Other Autostart Mechanisms":
    "AppInit DLLs, Session Manager BootExecute, and Windows Explorer group policy entries",
};

export const SOURCE_LABELS: Record<StartupSource, string> = {
  "registry-run-user": "Registry (HKCU Run)",
  "registry-run-machine": "Registry (HKLM Run)",
  "registry-run-once-user": "Registry (HKCU RunOnce)",
  "registry-run-once-machine": "Registry (HKLM RunOnce)",
  "startup-folder-user": "Startup Folder (User)",
  "startup-folder-machine": "Startup Folder (All Users)",
  "scheduled-task": "Scheduled Task",
  "windows-service": "Windows Service",
  "winlogon-shell": "Winlogon Shell",
  "winlogon-userinit": "Winlogon Userinit",
  "winlogon-notify": "Winlogon Notify",
  "app-init": "AppInit DLL",
  "boot-execute": "Boot Execute",
  "policy-run": "Explorer Policy Run",
};
