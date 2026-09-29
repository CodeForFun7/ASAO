import type {
  LoadImpactLevel,
  ProcessActivityState,
  ProcessCategory,
  ProcessStatus,
} from "./process";

export type SystemConditionState = "GOOD" | "ELEVATED" | "ATTENTION";

export type RecommendationPriority = "normal" | "interesting" | "important";

export type WidgetMode = "monitor" | "chat";

export type WidgetPositionPreset =
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right"
  | "custom";

export interface AsaoSettings {
  startWithWindows: boolean;
  widgetEnabled: boolean;
  launchWidgetOnStartup: boolean;
  alwaysOnTop: boolean;
  showRecommendations: boolean;
  notifications: boolean;
  widgetPosition: WidgetPositionPreset;
  customX: number | null;
  customY: number | null;
  widgetOpacity: number; // 60..100
}

export interface CompactProcessContext {
  pid: number;
  name: string;
  category: ProcessCategory;
  cpuPercent: number;
  sustainedCpuPercent?: number;
  memoryBytes: number;
  status: ProcessStatus;
  activityState?: ProcessActivityState;
  sustainedLoadSeconds?: number;
  impactLevel?: LoadImpactLevel;
}

export type RecommendationCategory = "process" | "startup" | "storage";

export interface WidgetRecommendation {
  id: string;
  category?: RecommendationCategory;
  subCategory?: string | null;
  title: string;
  message: string;
  priority: RecommendationPriority;
  actionLabel?: string | null;
  processPid: number | null;
  processName: string | null;
  startupItemId?: string | null;
  storagePath?: string | null;
  metricHighlight: string | null;
  impactLevel?: LoadImpactLevel | null;
  sustainedSeconds?: number | null;
}

export interface CategorizedRecommendations {
  processRecommendations: WidgetRecommendation[];
  startupRecommendations: WidgetRecommendation[];
  storageRecommendations: WidgetRecommendation[];
  combinedRecommendations: WidgetRecommendation[];
  timestampMs: number;
}

export interface WidgetSystemUpdate {
  cpuUsage: number;
  memoryUsage: number;
  memoryUsedBytes: number;
  memoryTotalBytes: number;
  gpuUsage: number;
  foregroundLoad?: number;
  backgroundLoad?: number;
  systemStrain?: number;
  userActive?: boolean;
  foregroundProcessName?: string | null;
  processCount: number;
  attentionCount: number;
  condition: SystemConditionState;
  conditionReason: string;
  recommendation: WidgetRecommendation;
  recommendations?: WidgetRecommendation[];
  topCpuProcesses: CompactProcessContext[];
  topMemoryProcesses: CompactProcessContext[];
  timestamp: number;
}

export interface WidgetHistorySample {
  timestamp: number;
  cpuUsage: number;
  memoryUsage: number;
}

export interface StructuredSystemContext {
  cpuUsage: number;
  ramUsage: number;
  memoryUsedBytes: number;
  memoryTotalBytes: number;
  gpuUsage: number;
  foregroundLoad: number;
  backgroundLoad: number;
  systemStrain: number;
  userActive: boolean;
  foregroundProcessName: string | null;
  processCount: number;
  attentionCount: number;
  systemCondition: SystemConditionState;
  conditionReason: string;
  topCpuProcesses: CompactProcessContext[];
  topMemoryProcesses: CompactProcessContext[];
  recentResourceHistory: WidgetHistorySample[];
  activeRecommendation: WidgetRecommendation | null;
}

export interface ChatMessage {
  id: string;
  sender: "user" | "asao";
  text: string;
  contextSummary?: string;
  timestamp: number;
}
