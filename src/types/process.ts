export type ProcessCategory =
  | "windows-core"
  | "drivers"
  | "gaming"
  | "development"
  | "productivity"
  | "communication"
  | "browser"
  | "unknown";

export type ProcessStatus =
  | "normal"
  | "active"
  | "high-resource"
  | "background"
  | "attention"
  | "protected";

export type ResourceLevel = "normal" | "moderate" | "high" | "very-high";

export interface ProcessInfo {
  pid: number;
  name: string;
  executablePath: string | null;
  parentPid: number | null;
  publisher: string | null;
  productName: string | null;
  cpuPercent: number;
  memoryBytes: number;
  diskBytesPerSec: number;
  networkBytesPerSec: number;
  threadCount: number;
  category: ProcessCategory;
  status: ProcessStatus;
  cpuLevel: ResourceLevel;
  memoryLevel: ResourceLevel;
  isStartup: boolean;
  isSystemCritical: boolean;
  isRestricted: boolean;
  startedSecondsAgo: number | null;
  description: string | null;
}

export interface SystemMetrics {
  cpuUsagePercent: number;
  cpuDeltaPercent: number;
  memoryUsagePercent: number;
  memoryUsedBytes: number;
  memoryTotalBytes: number;
  memoryDeltaPercent: number;
  gpuUsagePercent: number;
  gpuDeltaPercent: number;
  totalProcesses: number;
  attentionProcesses: number;
  protectedProcesses: number;
  highResourceProcesses: number;
  systemStatus: "healthy" | "warning" | "critical";
  timestampMs: number;
}

export interface SystemTelemetryPoint {
  timestamp: number;
  cpuPercent: number;
  memoryPercent: number;
  gpuPercent: number;
}

export interface ProcessSnapshotPayload {
  processes: ProcessInfo[];
  metrics: SystemMetrics;
}

export interface ProcessResourceSample {
  timestamp: number;
  cpuPercent: number;
  memoryBytes: number;
  diskBytesPerSec: number;
  networkBytesPerSec: number;
}

export interface ProcessAnalysis {
  summary: string;
  reason: string;
  recommendation?: string;
  risk?: "low" | "medium" | "high";
  confidence?: number;
  estimatedImpact?: string;
  isPlaceholder?: boolean;
}

export interface ProcessAnalysisService {
  analyzeProcess(process: ProcessInfo): Promise<ProcessAnalysis>;
  analyzeSystem(
    metrics: SystemMetrics,
    attentionProcesses: ProcessInfo[]
  ): Promise<ProcessAnalysis>;
}

export type SortOption =
  | "resource-desc"
  | "cpu-desc"
  | "cpu-asc"
  | "memory-desc"
  | "memory-asc"
  | "name-asc"
  | "name-desc"
  | "category"
  | "status";

export type ResourceUsageFilter =
  | "any"
  | "cpu-25"
  | "cpu-50"
  | "ram-500"
  | "ram-1024";

export type CriticalityFilter = "all" | "system-critical" | "user-space";

export type AppRoute = "dashboard" | "processes" | "settings";

export const CATEGORY_METADATA: Record<
  ProcessCategory | "all",
  { label: string; shortLabel: string; description: string }
> = {
  all: {
    label: "All Processes",
    shortLabel: "All",
    description: "Every active user and system process enumerated on this machine",
  },
  "windows-core": {
    label: "Windows Core",
    shortLabel: "Windows",
    description: "Core operating system services, kernel hosts, and shell components",
  },
  drivers: {
    label: "Drivers",
    shortLabel: "Drivers",
    description: "Hardware device drivers, GPU containers, and audio/input daemons",
  },
  gaming: {
    label: "Gaming",
    shortLabel: "Gaming",
    description: "Game launchers, overlays, and gaming runtime services",
  },
  development: {
    label: "Development",
    shortLabel: "Development",
    description: "IDEs, compilers, language servers, terminals, and local runtimes",
  },
  productivity: {
    label: "Productivity",
    shortLabel: "Productivity",
    description: "Document suites, cloud sync engines, notes, and creative tools",
  },
  communication: {
    label: "Communication",
    shortLabel: "Communication",
    description: "Messaging, voice, and video conferencing applications",
  },
  browser: {
    label: "Browser Components",
    shortLabel: "Browser",
    description: "Web browsers, renderer sandboxes, and WebView2 hosts",
  },
  unknown: {
    label: "Unknown",
    shortLabel: "Unknown",
    description: "Processes without matching catalog signatures or vendor rules",
  },
};

export const STATUS_METADATA: Record<
  ProcessStatus,
  { label: string; dotColor: string; badgeBg: string; badgeText: string; badgeBorder: string }
> = {
  normal: {
    label: "Normal",
    dotColor: "bg-lunar-healthy",
    badgeBg: "bg-lunar-healthy/10",
    badgeText: "text-lunar-healthy",
    badgeBorder: "border-lunar-healthy/25",
  },
  active: {
    label: "Active",
    dotColor: "bg-lunar-white",
    badgeBg: "bg-lunar-white/10",
    badgeText: "text-lunar-white",
    badgeBorder: "border-lunar-white/25",
  },
  "high-resource": {
    label: "High Resource Usage",
    dotColor: "bg-lunar-warning",
    badgeBg: "bg-lunar-warning/10",
    badgeText: "text-lunar-warning",
    badgeBorder: "border-lunar-warning/30",
  },
  background: {
    label: "Background",
    dotColor: "bg-lunar-muted",
    badgeBg: "bg-lunar-surface-2",
    badgeText: "text-lunar-text-sec",
    badgeBorder: "border-lunar-border",
  },
  attention: {
    label: "Needs Attention",
    dotColor: "bg-lunar-critical",
    badgeBg: "bg-lunar-critical/15",
    badgeText: "text-lunar-critical",
    badgeBorder: "border-lunar-critical/35",
  },
  protected: {
    label: "Protected",
    dotColor: "bg-lunar-ai",
    badgeBg: "bg-lunar-ai/10",
    badgeText: "text-lunar-ai",
    badgeBorder: "border-lunar-ai/25",
  },
};
