import type { AppRoute } from "./process";

export type DiagnosticStepStatus = "pending" | "running" | "completed" | "error";

export interface DiagnosticStep {
  id: string;
  label: string;
  toolName?: string;
  status: DiagnosticStepStatus;
  detail?: string;
  startedAt?: number;
  durationMs?: number;
}

export type FixActionType =
  | "disable_startup"
  | "enable_startup"
  | "inspect_process"
  | "open_storage"
  | "navigate_route";

export interface ActionableFix {
  id: string;
  title: string;
  description: string;
  actionType: FixActionType;
  payload: {
    startupId?: string;
    startupName?: string;
    source?: string;
    registryKey?: string | null;
    serviceName?: string | null;
    taskPath?: string | null;
    commandLine?: string | null;
    pid?: number;
    processName?: string;
    path?: string;
    route?: AppRoute;
  };
  status: "idle" | "applying" | "applied" | "failed";
  feedbackMessage?: string;
}

export type VisualWidgetType =
  | "pills"
  | "resource_chart"
  | "process_chart"
  | "startup_chart"
  | "storage_card"
  | "actionable_fixes";

export type MetricPillType = "cpu" | "ram" | "disk" | "startup";

export interface DiagnosticVisualData {
  queryType?: "general_slow" | "startup_delay" | "process_memory" | "storage_space" | "system_health";
  visibleWidgets?: VisualWidgetType[];
  visiblePills?: MetricPillType[];
  overview?: {
    cpuUsagePercent: number;
    memoryUsagePercent: number;
    memoryUsed: string;
    memoryTotal: string;
    gpuUsagePercent?: number;
    foregroundLoadPercent?: number;
    backgroundLoadPercent?: number;
    systemStrainPercent: number;
    condition: "healthy" | "warning" | "critical" | string;
    conditionReason?: string;
    foregroundProcessName?: string;
  };
  startup?: {
    totalStartupItems: number;
    highImpactCount: number;
    disabledCount: number;
    runningCount: number;
    estimatedBootDelayMs: number;
    highImpactItems: Array<{
      id: string;
      name: string;
      impact: "high" | "medium" | "low";
      bootDurationMs: number;
      bootCpuMs: number;
      bootDiskBytes: string;
      memoryBytes: string;
      isEnabled: boolean;
      isCurrentlyRunning: boolean;
      recommendation: string;
      classification: string;
      disableMethod?: string;
      source?: string;
    }>;
    sampledItems?: Array<{
      id: string;
      name: string;
      impact: string;
      isEnabled: boolean;
      bootDurationMs: number;
    }>;
  };
  processes?: {
    totalProcesses: number;
    backgroundLoadPercent: number;
    foregroundLoadPercent: number;
    systemStrainPercent: number;
    topProcesses: Array<{
      pid: number;
      name: string;
      category: string;
      status: string;
      activityState: string;
      cpuPercent: number;
      sustainedCpuPercent: number;
      memoryFormatted: string;
      memoryBytes: number;
      isSystemCritical: boolean;
      backgroundImpactScore: number;
    }>;
  };
  storage?: {
    selectedDrive: string;
    drives: Array<{
      drive: string;
      mountPoint: string;
      total: string;
      used: string;
      free: string;
      usagePercentage: number;
    }>;
    categories: Array<{
      category: string;
      label: string;
      sizeFormatted: string;
      percentage: number;
    }>;
    largestFiles: Array<{
      name: string;
      path: string;
      sizeFormatted: string;
      category: string;
    }>;
    largestFolders: Array<{
      name: string;
      path: string;
      sizeFormatted: string;
      percentage: number;
    }>;
  };
  findings?: string[];
  recommendedActions?: string[];
}

export interface ChatMessage {
  id: string;
  sender: "user" | "assistant";
  text: string;
  timestamp: number;
  steps?: DiagnosticStep[];
  fixes?: ActionableFix[];
  diagnosticData?: DiagnosticVisualData;
  isStreaming?: boolean;
  error?: string | null;
}

export interface VertexAgentConfig {
  projectId: string;
  location: string;
  model: string;
  apiKey: string;
  authToken?: string;
  useLocalFallbackIfUnconfigured: boolean;
}

export interface ToolCallDeclaration {
  name: string;
  description: string;
  parameters: {
    type: "OBJECT";
    properties: Record<string, {
      type: string;
      description: string;
      enum?: string[];
    }>;
    required?: string[];
  };
}
