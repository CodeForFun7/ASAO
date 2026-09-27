import { create } from "zustand";
import type {
  AppRoute,
  CriticalityFilter,
  ProcessCategory,
  ProcessInfo,
  ProcessResourceSample,
  ProcessStatus,
  ResourceUsageFilter,
  SortOption,
  SystemMetrics,
  SystemTelemetryPoint,
} from "../types/process";
import {
  getFullSnapshot,
  startMonitoring,
  stopMonitoring,
  subscribeToTelemetry,
} from "../services/tauri";

const MAX_HISTORY_SAMPLES = 30; // 30 seconds at 1s interval

interface ProcessStoreState {
  // Navigation
  currentRoute: AppRoute;
  setRoute: (route: AppRoute) => void;

  // Live Telemetry Data
  processes: ProcessInfo[];
  systemMetrics: SystemMetrics | null;
  systemHistory: SystemTelemetryPoint[];
  selectedProcessPid: number | null;
  resourceHistory: Record<number, ProcessResourceSample[]>;

  // Search, Filter & Sort Controls
  searchQuery: string;
  categoryFilter: ProcessCategory | "all";
  multiCategoryFilter: ProcessCategory[];
  statusFilter: ProcessStatus[];
  resourceFilter: ResourceUsageFilter;
  criticalityFilter: CriticalityFilter;
  sort: SortOption;

  // Monitoring State
  monitoringStatus: "loading" | "active" | "paused" | "error";
  errorMessage: string | null;

  // Actions
  selectProcess: (pid: number | null) => void;
  setSearchQuery: (query: string) => void;
  setCategoryFilter: (category: ProcessCategory | "all") => void;
  toggleMultiCategory: (category: ProcessCategory) => void;
  toggleStatusFilter: (status: ProcessStatus) => void;
  setResourceFilter: (filter: ResourceUsageFilter) => void;
  setCriticalityFilter: (filter: CriticalityFilter) => void;
  setSort: (sort: SortOption) => void;
  resetFilters: () => void;

  // Dashboard Deep-Link Navigators
  navigateToAttentionProcesses: () => void;
  navigateToMemoryProcesses: () => void;
  navigateToCpuProcesses: () => void;

  // Telemetry Lifecycle
  initializeMonitoring: () => Promise<() => void>;
  retryMonitoring: () => Promise<void>;
  togglePauseMonitoring: () => Promise<void>;
}

export const useProcessStore = create<ProcessStoreState>((set, get) => ({
  currentRoute: "dashboard",
  setRoute: (route) => set({ currentRoute: route }),

  processes: [],
  systemMetrics: null,
  systemHistory: [],
  selectedProcessPid: null,
  resourceHistory: {},

  searchQuery: "",
  categoryFilter: "all",
  multiCategoryFilter: [],
  statusFilter: [],
  resourceFilter: "any",
  criticalityFilter: "all",
  sort: "resource-desc",

  monitoringStatus: "loading",
  errorMessage: null,

  selectProcess: (pid) => set({ selectedProcessPid: pid }),

  setSearchQuery: (query) => set({ searchQuery: query }),

  setCategoryFilter: (category) =>
    set({
      categoryFilter: category,
      multiCategoryFilter: [],
    }),

  toggleMultiCategory: (category) =>
    set((state) => {
      const exists = state.multiCategoryFilter.includes(category);
      const next = exists
        ? state.multiCategoryFilter.filter((c) => c !== category)
        : [...state.multiCategoryFilter, category];
      return {
        multiCategoryFilter: next,
        categoryFilter: next.length === 1 ? next[0] : "all",
      };
    }),

  toggleStatusFilter: (status) =>
    set((state) => {
      const exists = state.statusFilter.includes(status);
      const next = exists
        ? state.statusFilter.filter((s) => s !== status)
        : [...state.statusFilter, status];
      return { statusFilter: next };
    }),

  setResourceFilter: (filter) => set({ resourceFilter: filter }),

  setCriticalityFilter: (filter) => set({ criticalityFilter: filter }),

  setSort: (sort) => set({ sort }),

  resetFilters: () =>
    set({
      searchQuery: "",
      categoryFilter: "all",
      multiCategoryFilter: [],
      statusFilter: [],
      resourceFilter: "any",
      criticalityFilter: "all",
      sort: "resource-desc",
    }),

  navigateToAttentionProcesses: () =>
    set({
      currentRoute: "processes",
      categoryFilter: "all",
      multiCategoryFilter: [],
      statusFilter: ["attention"],
      resourceFilter: "any",
      criticalityFilter: "all",
      sort: "resource-desc",
    }),

  navigateToMemoryProcesses: () =>
    set({
      currentRoute: "processes",
      categoryFilter: "all",
      multiCategoryFilter: [],
      statusFilter: [],
      resourceFilter: "any",
      criticalityFilter: "all",
      sort: "memory-desc",
    }),

  navigateToCpuProcesses: () =>
    set({
      currentRoute: "processes",
      categoryFilter: "all",
      multiCategoryFilter: [],
      statusFilter: [],
      resourceFilter: "any",
      criticalityFilter: "all",
      sort: "cpu-desc",
    }),

  initializeMonitoring: async () => {
    set({ monitoringStatus: "loading", errorMessage: null });

    const applyProcessSnapshot = (incomingProcesses: ProcessInfo[]) => {
      const now = Date.now();
      const prevHistory = get().resourceHistory;
      const nextHistory: Record<number, ProcessResourceSample[]> = {};

      for (const proc of incomingProcesses) {
        const existing = prevHistory[proc.pid] ?? [];
        const sample: ProcessResourceSample = {
          timestamp: now,
          cpuPercent: proc.cpuPercent,
          memoryBytes: proc.memoryBytes,
          diskBytesPerSec: proc.diskBytesPerSec,
          networkBytesPerSec: proc.networkBytesPerSec,
        };
        const updated =
          existing.length >= MAX_HISTORY_SAMPLES
            ? [...existing.slice(existing.length - MAX_HISTORY_SAMPLES + 1), sample]
            : [...existing, sample];
        nextHistory[proc.pid] = updated;
      }

      const currentSelected = get().selectedProcessPid;
      const stillExists =
        currentSelected !== null &&
        incomingProcesses.some((p) => p.pid === currentSelected);

      set({
        processes: incomingProcesses,
        resourceHistory: nextHistory,
        selectedProcessPid: stillExists ? currentSelected : currentSelected,
        monitoringStatus:
          get().monitoringStatus === "paused" ? "paused" : "active",
        errorMessage: null,
      });
    };

    const applySystemMetrics = (metrics: SystemMetrics) => {
      const prevSys = get().systemHistory;
      const procs = get().processes;
      const fallbackDisk = procs.reduce(
        (acc, p) => acc + (p.diskBytesPerSec || 0),
        0
      );
      const fallbackNet = procs.reduce(
        (acc, p) => acc + (p.networkBytesPerSec || 0),
        0
      );
      const point: SystemTelemetryPoint = {
        timestamp: metrics.timestampMs || Date.now(),
        cpuPercent: metrics.cpuUsagePercent,
        memoryPercent: metrics.memoryUsagePercent,
        gpuPercent: metrics.gpuUsagePercent ?? 0,
        diskBytesPerSec: metrics.diskBytesPerSec ?? fallbackDisk,
        networkBytesPerSec: metrics.networkBytesPerSec ?? fallbackNet,
      };
      const nextSys =
        prevSys.length >= MAX_HISTORY_SAMPLES
          ? [...prevSys.slice(prevSys.length - MAX_HISTORY_SAMPLES + 1), point]
          : [...prevSys, point];

      set({
        systemMetrics: {
          ...metrics,
          diskBytesPerSec: metrics.diskBytesPerSec ?? fallbackDisk,
          networkBytesPerSec: metrics.networkBytesPerSec ?? fallbackNet,
        },
        systemHistory: nextSys,
        monitoringStatus:
          get().monitoringStatus === "paused" ? "paused" : "active",
      });
    };

    try {
      const initial = await getFullSnapshot();
      applyProcessSnapshot(initial.processes);
      applySystemMetrics(initial.metrics);
      set({ errorMessage: null });
    } catch (err) {
      set({
        monitoringStatus: "error",
        errorMessage:
          err instanceof Error
            ? err.message
            : "Unable to read process information.",
      });
    }

    const cleanupListeners = await subscribeToTelemetry({
      onProcessSnapshot: (procs) => {
        if (get().monitoringStatus !== "paused") {
          applyProcessSnapshot(procs);
        }
      },
      onSystemMetrics: (metrics) => {
        if (get().monitoringStatus !== "paused") {
          applySystemMetrics(metrics);
        }
      },
      onError: (errMsg) => {
        set({ monitoringStatus: "error", errorMessage: errMsg });
      },
    });

    try {
      await startMonitoring();
    } catch {
      // Non-fatal if initial snapshot already succeeded
    }

    return () => {
      cleanupListeners();
    };
  },

  retryMonitoring: async () => {
    set({ monitoringStatus: "loading", errorMessage: null });
    try {
      const snapshot = await getFullSnapshot();
      set({
        processes: snapshot.processes,
        systemMetrics: snapshot.metrics,
        monitoringStatus: "active",
        errorMessage: null,
      });
      await startMonitoring();
    } catch (err) {
      set({
        monitoringStatus: "error",
        errorMessage:
          err instanceof Error
            ? err.message
            : "Unable to read process information.",
      });
    }
  },

  togglePauseMonitoring: async () => {
    const status = get().monitoringStatus;
    if (status === "active") {
      await stopMonitoring();
      set({ monitoringStatus: "paused" });
    } else {
      await startMonitoring();
      set({ monitoringStatus: "active" });
    }
  },
}));
