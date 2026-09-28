export type StorageCategory =
  | "SYSTEM"
  | "APPLICATION"
  | "USER"
  | "CACHE"
  | "TEMPORARY"
  | "UNKNOWN";

export type StorageImportance =
  | "CRITICAL"
  | "IMPORTANT"
  | "NORMAL"
  | "LOW"
  | "UNKNOWN";

export interface StorageDrive {
  drive: string;
  mountPoint: string;
  name: string;
  fileSystem: string;
  driveType: string;
  totalCapacity: number;
  usedCapacity: number;
  freeCapacity: number;
  usagePercentage: number;
}

export interface StorageItem {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  itemType: string;
  extension: string;
  category: StorageCategory;
  importance: StorageImportance;
  createdMs: number | null;
  modifiedMs: number | null;
  accessedMs: number | null;
  analysisExplanation: string;
}

export interface StorageCategorySlice {
  category: StorageCategory;
  label: string;
  bytes: number;
  percentage: number;
  itemCount: number;
}

export interface StorageInsightAffectedItem {
  name: string;
  path: string;
  size: number;
  isDir: boolean;
  category: StorageCategory;
  importance: StorageImportance;
}

export interface StorageInsight {
  id: string;
  insightType: string;
  title: string;
  description: string;
  size: number;
  affectedItems: StorageInsightAffectedItem[];
  importance: StorageImportance;
  action: string;
  targetPath: string | null;
  targetCategory: StorageCategory | null;
}

export interface StorageScanProgress {
  isScanning: boolean;
  drive: string;
  currentPath: string;
  filesAnalyzed: number;
  foldersAnalyzed: number;
  bytesAnalyzed: number;
  totalUsedBytes: number;
  progressPercent: number;
}

export interface StorageDirectoryListing {
  path: string;
  parentPath: string | null;
  items: StorageItem[];
  permissionDenied: boolean;
  errorMessage: string | null;
}

export interface StorageAnalysisSnapshot {
  selectedDrive: string;
  drives: StorageDrive[];
  distribution: StorageCategorySlice[];
  insights: StorageInsight[];
  largestFolders: StorageItem[];
  largestFiles: StorageItem[];
  discoveredExtensions: string[];
  initialDirectory: StorageDirectoryListing;
  filesAnalyzed: number;
  foldersAnalyzed: number;
  bytesAnalyzed: number;
  scanTimestampMs: number;
}

export type StorageSortField =
  | "size"
  | "name"
  | "modified"
  | "accessed"
  | "category"
  | "importance";

export type StorageSortDirection = "desc" | "asc";

export type LargestFilesSortField = "size" | "modified" | "accessed";

export const STORAGE_CATEGORY_META: Record<
  StorageCategory,
  {
    label: string;
    shortLabel: string;
    strokeColor: string;
    dotColor: string;
    badgeClass: string;
  }
> = {
  SYSTEM: {
    label: "System",
    shortLabel: "SYSTEM",
    strokeColor: "#F2F2F2",
    dotColor: "bg-[#F2F2F2]",
    badgeClass: "text-lunar-white",
  },
  APPLICATION: {
    label: "Applications",
    shortLabel: "APPLICATION",
    strokeColor: "#A6A1B8",
    dotColor: "bg-[#A6A1B8]",
    badgeClass: "text-lunar-ai",
  },
  USER: {
    label: "User Files",
    shortLabel: "USER",
    strokeColor: "#2EB872",
    dotColor: "bg-[#2EB872]",
    badgeClass: "text-lunar-healthy",
  },
  CACHE: {
    label: "Cache",
    shortLabel: "CACHE",
    strokeColor: "#EAB308",
    dotColor: "bg-[#EAB308]",
    badgeClass: "text-lunar-warning",
  },
  TEMPORARY: {
    label: "Temporary Files",
    shortLabel: "TEMPORARY",
    strokeColor: "#3B82F6",
    dotColor: "bg-[#3B82F6]",
    badgeClass: "text-[#3B82F6]",
  },
  UNKNOWN: {
    label: "Other / Unknown",
    shortLabel: "UNKNOWN",
    strokeColor: "#5E5E5E",
    dotColor: "bg-[#5E5E5E]",
    badgeClass: "text-lunar-text-sec",
  },
};

export const STORAGE_IMPORTANCE_META: Record<
  StorageImportance,
  {
    label: string;
    priority: number;
    dotColor: string;
    badgeClass: string;
  }
> = {
  CRITICAL: {
    label: "CRITICAL",
    priority: 0,
    dotColor: "bg-lunar-critical",
    badgeClass: "text-lunar-critical",
  },
  IMPORTANT: {
    label: "IMPORTANT",
    priority: 1,
    dotColor: "bg-[#3B82F6]",
    badgeClass: "text-[#3B82F6]",
  },
  NORMAL: {
    label: "NORMAL",
    priority: 2,
    dotColor: "bg-lunar-white",
    badgeClass: "text-lunar-white",
  },
  LOW: {
    label: "LOW",
    priority: 3,
    dotColor: "bg-lunar-text-sec",
    badgeClass: "text-lunar-text-sec",
  },
  UNKNOWN: {
    label: "UNKNOWN",
    priority: 4,
    dotColor: "bg-lunar-text-sec",
    badgeClass: "text-lunar-text-sec",
  },
};
