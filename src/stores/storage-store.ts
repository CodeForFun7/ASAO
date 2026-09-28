import { create } from "zustand";
import type {
  LargestFilesSortField,
  StorageAnalysisSnapshot,
  StorageCategory,
  StorageDirectoryListing,
  StorageDrive,
  StorageImportance,
  StorageInsight,
  StorageItem,
  StorageScanProgress,
  StorageSortDirection,
  StorageSortField,
} from "../types/storage";
import {
  cancelStorageScan,
  getStorageDrives,
  getStorageSnapshot,
  openStorageLocation,
  readStorageDirectory,
  searchStorageItems,
  startStorageScan,
  subscribeToStorageScanProgress,
} from "../services/storage";

export type StorageScanState = "unscanned" | "scanning" | "ready" | "error";

interface StorageStoreState {
  drives: StorageDrive[];
  selectedDrive: string | null;
  scanState: StorageScanState;
  scanProgress: StorageScanProgress | null;
  snapshot: StorageAnalysisSnapshot | null;
  errorMessage: string | null;

  // Filesystem Explorer state
  currentPath: string;
  directoryListing: StorageDirectoryListing | null;
  isLoadingDirectory: boolean;
  searchQuery: string;
  searchResults: StorageItem[] | null;
  isSearching: boolean;

  categoryFilter: StorageCategory | "ALL";
  importanceFilter: StorageImportance | "ALL";
  typeFilter: string | "ALL";
  sortField: StorageSortField;
  sortDirection: StorageSortDirection;

  // Largest Files sort
  largestFilesSort: LargestFilesSortField;

  // Right-side File Details Inspector
  selectedItem: StorageItem | null;

  // Active Insight Review filter
  activeInsightReview: StorageInsight | null;

  // Actions
  initializeStorage: () => Promise<() => void>;
  selectDrive: (drive: string) => Promise<void>;
  triggerScan: (drive?: string) => Promise<void>;
  cancelScan: () => Promise<void>;
  navigateToDirectory: (path: string) => Promise<void>;
  retryCurrentDirectory: () => Promise<void>;
  setSearchQuery: (query: string) => void;
  setCategoryFilter: (cat: StorageCategory | "ALL") => void;
  setImportanceFilter: (imp: StorageImportance | "ALL") => void;
  setTypeFilter: (ext: string | "ALL") => void;
  setSort: (field: StorageSortField, direction?: StorageSortDirection) => void;
  setLargestFilesSort: (field: LargestFilesSortField) => void;
  selectItem: (item: StorageItem | null) => void;
  reviewInsight: (insight: StorageInsight) => Promise<void>;
  clearInsightReview: () => void;
  resetExplorerFilters: () => void;
  openLocationInExplorer: (path: string) => Promise<void>;
}

let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;

export const useStorageStore = create<StorageStoreState>((set, get) => ({
  drives: [],
  selectedDrive: null,
  scanState: "unscanned",
  scanProgress: null,
  snapshot: null,
  errorMessage: null,

  currentPath: "",
  directoryListing: null,
  isLoadingDirectory: false,
  searchQuery: "",
  searchResults: null,
  isSearching: false,

  categoryFilter: "ALL",
  importanceFilter: "ALL",
  typeFilter: "ALL",
  sortField: "size",
  sortDirection: "desc",

  largestFilesSort: "size",
  selectedItem: null,
  activeInsightReview: null,

  initializeStorage: async () => {
    let unlistenProgress: (() => void) | undefined;

    try {
      unlistenProgress = await subscribeToStorageScanProgress((progress) => {
        set({
          scanProgress: progress,
          scanState: progress.isScanning ? "scanning" : get().scanState,
        });
      });
    } catch {
      // Fallback if outside Tauri event context
    }

    try {
      const detectedDrives = await getStorageDrives();
      const currentSelected =
        get().selectedDrive ??
        (detectedDrives.length > 0 ? detectedDrives[0].drive : null);

      set({
        drives: detectedDrives,
        selectedDrive: currentSelected,
      });

      if (currentSelected) {
        const existingSnap = await getStorageSnapshot(currentSelected);
        if (existingSnap) {
          set({
            snapshot: existingSnap,
            drives:
              existingSnap.drives.length > 0
                ? existingSnap.drives
                : detectedDrives,
            scanState: "ready",
            currentPath:
              get().currentPath || existingSnap.initialDirectory.path,
            directoryListing:
              get().directoryListing || existingSnap.initialDirectory,
          });
        } else {
          // Automatically begin initial storage analysis for the active drive
          void get().triggerScan(currentSelected);
        }
      }
    } catch (err) {
      set({
        errorMessage:
          err instanceof Error
            ? err.message
            : "Failed to initialize storage subsystem.",
      });
    }

    return () => {
      if (unlistenProgress) {
        unlistenProgress();
      }
    };
  },

  selectDrive: async (drive: string) => {
    const { drives } = get();
    const target = drives.find(
      (d) =>
        d.drive.toUpperCase() === drive.toUpperCase() ||
        d.mountPoint.toUpperCase() === drive.toUpperCase()
    );
    const driveCode = target ? target.drive : drive;
    const mountPoint = target ? target.mountPoint : `${driveCode}\\`;

    set({
      selectedDrive: driveCode,
      selectedItem: null,
      activeInsightReview: null,
      searchQuery: "",
      searchResults: null,
      categoryFilter: "ALL",
      importanceFilter: "ALL",
      typeFilter: "ALL",
      currentPath: mountPoint,
    });

    try {
      const existingSnap = await getStorageSnapshot(driveCode);
      if (existingSnap) {
        set({
          snapshot: existingSnap,
          drives:
            existingSnap.drives.length > 0 ? existingSnap.drives : get().drives,
          scanState: "ready",
          currentPath: existingSnap.initialDirectory.path,
          directoryListing: existingSnap.initialDirectory,
          errorMessage: null,
        });
      } else {
        await get().triggerScan(driveCode);
      }
    } catch {
      await get().triggerScan(driveCode);
    }
  },

  triggerScan: async (drive?: string) => {
    const targetDrive = drive ?? get().selectedDrive ?? undefined;
    const matchedDrive = get().drives.find(
      (d) => d.drive.toUpperCase() === (targetDrive ?? "").toUpperCase()
    );

    set({
      scanState: "scanning",
      errorMessage: null,
      activeInsightReview: null,
      scanProgress: {
        isScanning: true,
        drive: targetDrive ?? "",
        currentPath: matchedDrive?.mountPoint ?? "",
        filesAnalyzed: 0,
        foldersAnalyzed: 0,
        bytesAnalyzed: 0,
        totalUsedBytes: matchedDrive?.usedCapacity ?? 0,
        progressPercent: 2,
      },
    });

    try {
      const snapshot = await startStorageScan(targetDrive);
      set({
        snapshot,
        drives: snapshot.drives.length > 0 ? snapshot.drives : get().drives,
        selectedDrive: snapshot.selectedDrive,
        scanState: "ready",
        scanProgress: null,
        currentPath: snapshot.initialDirectory.path,
        directoryListing: snapshot.initialDirectory,
        errorMessage: null,
      });
    } catch (err) {
      const msg =
        typeof err === "string"
          ? err
          : err instanceof Error
          ? err.message
          : "Storage scan could not be completed.";

      if (msg.toLowerCase().includes("cancel")) {
        set({
          scanState: get().snapshot ? "ready" : "unscanned",
          scanProgress: null,
        });
      } else {
        set({
          scanState: "error",
          scanProgress: null,
          errorMessage: msg,
        });
      }
    }
  },

  cancelScan: async () => {
    try {
      await cancelStorageScan();
    } finally {
      set({
        scanState: get().snapshot ? "ready" : "unscanned",
        scanProgress: null,
      });
    }
  },

  navigateToDirectory: async (path: string) => {
    if (!path) return;
    set({
      isLoadingDirectory: true,
      currentPath: path,
      activeInsightReview: null,
      searchQuery: "",
      searchResults: null,
    });

    try {
      const listing = await readStorageDirectory(
        path,
        get().selectedDrive ?? undefined
      );
      set({
        directoryListing: listing,
        currentPath: listing.path,
        isLoadingDirectory: false,
      });
    } catch (err) {
      set({
        directoryListing: {
          path,
          parentPath: null,
          items: [],
          permissionDenied: true,
          errorMessage:
            err instanceof Error
              ? err.message
              : "Unable to read this directory.",
        },
        isLoadingDirectory: false,
      });
    }
  },

  retryCurrentDirectory: async () => {
    const { currentPath } = get();
    if (currentPath) {
      await get().navigateToDirectory(currentPath);
    }
  },

  setSearchQuery: (query: string) => {
    set({ searchQuery: query });
    if (searchDebounceTimer) {
      clearTimeout(searchDebounceTimer);
    }

    const trimmed = query.trim();
    if (!trimmed) {
      set({ searchResults: null, isSearching: false });
      return;
    }

    set({ isSearching: true });
    searchDebounceTimer = setTimeout(async () => {
      try {
        const results = await searchStorageItems(
          trimmed,
          get().selectedDrive ?? undefined,
          get().currentPath || undefined
        );
        if (get().searchQuery.trim() === trimmed) {
          set({ searchResults: results, isSearching: false });
        }
      } catch {
        if (get().searchQuery.trim() === trimmed) {
          set({ searchResults: [], isSearching: false });
        }
      }
    }, 180);
  },

  setCategoryFilter: (cat) => set({ categoryFilter: cat }),

  setImportanceFilter: (imp) => set({ importanceFilter: imp }),

  setTypeFilter: (ext) => set({ typeFilter: ext }),

  setSort: (field, direction) => {
    const currentField = get().sortField;
    const currentDir = get().sortDirection;
    const nextDir =
      direction ??
      (currentField === field
        ? currentDir === "desc"
          ? "asc"
          : "desc"
        : field === "name"
        ? "asc"
        : "desc");
    set({ sortField: field, sortDirection: nextDir });
  },

  setLargestFilesSort: (field) => set({ largestFilesSort: field }),

  selectItem: (item) => set({ selectedItem: item }),

  reviewInsight: async (insight) => {
    set({
      activeInsightReview: insight,
      searchQuery: "",
      searchResults: null,
      categoryFilter: insight.targetCategory ?? "ALL",
      importanceFilter: "ALL",
      typeFilter: "ALL",
    });

    if (insight.targetPath) {
      // Check if targetPath is a directory or file
      const firstItem = insight.affectedItems[0];
      if (firstItem && !firstItem.isDir && insight.targetPath === firstItem.path) {
        const lastSep = Math.max(
          insight.targetPath.lastIndexOf("\\"),
          insight.targetPath.lastIndexOf("/")
        );
        if (lastSep > 2) {
          const parentDir = insight.targetPath.slice(0, lastSep);
          await get().navigateToDirectory(parentDir);
          set({ activeInsightReview: insight });
        }
      } else {
        await get().navigateToDirectory(insight.targetPath);
        set({ activeInsightReview: insight });
      }
    }
  },

  clearInsightReview: () =>
    set({
      activeInsightReview: null,
      categoryFilter: "ALL",
    }),

  resetExplorerFilters: () =>
    set({
      searchQuery: "",
      searchResults: null,
      categoryFilter: "ALL",
      importanceFilter: "ALL",
      typeFilter: "ALL",
      activeInsightReview: null,
    }),

  openLocationInExplorer: async (path: string) => {
    try {
      await openStorageLocation(path);
    } catch {
      // Ignore if path inaccessible
    }
  },
}));
