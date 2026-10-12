import { create } from "zustand";
import type {
  BootPerformanceSummary,
  Recommendation,
  StartupGroup,
  StartupImpact,
  StartupItem,
  WprStatus,
} from "../types/startup";
import {
  cancelWprBootTrace,
  disableStartupItem,
  enableStartupItem,
  getBootPerformanceSummary,
  getStartupItems,
  getWprStatus,
  startWprBootTrace,
} from "../services/startup";

interface StartupStoreState {
  items: StartupItem[];
  summary: BootPerformanceSummary | null;
  wprStatus: WprStatus | null;
  selectedItemId: string | null;
  loading: boolean;
  actionInProgressId: string | null;
  error: string | null;

  // Filters & Search
  searchQuery: string;
  filterImpact: StartupImpact | "all";
  filterRecommendation: Recommendation | "all";
  filterRunning: "all" | "running" | "stopped";
  filterEnabled: "all" | "enabled" | "disabled";
  expandedGroups: Record<StartupGroup, boolean>;

  // Actions
  loadStartupData: () => Promise<void>;
  selectItem: (id: string | null) => void;
  toggleGroup: (group: StartupGroup) => void;
  expandAllGroups: () => void;
  collapseAllGroups: () => void;
  toggleItemState: (item: StartupItem) => Promise<boolean>;
  refreshWprStatus: () => Promise<void>;
  startBootTrace: () => Promise<string>;
  cancelBootTrace: () => Promise<string>;

  setSearchQuery: (query: string) => void;
  setFilterImpact: (impact: StartupImpact | "all") => void;
  setFilterRecommendation: (rec: Recommendation | "all") => void;
  setFilterRunning: (running: "all" | "running" | "stopped") => void;
  setFilterEnabled: (enabled: "all" | "enabled" | "disabled") => void;
  resetFilters: () => void;
}

const DEFAULT_EXPANDED: Record<StartupGroup, boolean> = {
  "Registry Startup": true,
  "Startup Folder": true,
  "Scheduled Tasks": true,
  "Windows Services": false,
  "Winlogon / System Startup": false,
  "Other Autostart Mechanisms": false,
};

export const useStartupStore = create<StartupStoreState>((set, get) => ({
  items: [],
  summary: null,
  wprStatus: null,
  selectedItemId: null,
  loading: false,
  actionInProgressId: null,
  error: null,

  searchQuery: "",
  filterImpact: "all",
  filterRecommendation: "all",
  filterRunning: "all",
  filterEnabled: "all",
  expandedGroups: DEFAULT_EXPANDED,

  loadStartupData: async () => {
    set({ loading: true, error: null });
    try {
      const [items, summary, wprStatus] = await Promise.all([
        getStartupItems(),
        getBootPerformanceSummary().catch(() => null),
        getWprStatus().catch(() => null),
      ]);

      set({
        items,
        summary,
        wprStatus,
        loading: false,
        // If an item was selected, ensure it still exists; otherwise keep or select first high-impact item
        selectedItemId: get().selectedItemId ?? (items.find((i) => i.impact === "high")?.id ?? items[0]?.id ?? null),
      });
    } catch (err) {
      set({
        loading: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  selectItem: (id) => set({ selectedItemId: id }),

  toggleGroup: (group) =>
    set((state) => ({
      expandedGroups: {
        ...state.expandedGroups,
        [group]: !state.expandedGroups[group],
      },
    })),

  expandAllGroups: () =>
    set({
      expandedGroups: {
        "Registry Startup": true,
        "Startup Folder": true,
        "Scheduled Tasks": true,
        "Windows Services": true,
        "Winlogon / System Startup": true,
        "Other Autostart Mechanisms": true,
      },
    }),

  collapseAllGroups: () =>
    set({
      expandedGroups: {
        "Registry Startup": false,
        "Startup Folder": false,
        "Scheduled Tasks": false,
        "Windows Services": false,
        "Winlogon / System Startup": false,
        "Other Autostart Mechanisms": false,
      },
    }),

  toggleItemState: async (item) => {
    set({ actionInProgressId: item.id, error: null });
    const isCurrentlyEnabled = item.isEnabled;
    try {
      if (isCurrentlyEnabled) {
        await disableStartupItem(
          item.id,
          item.source,
          item.registryKey,
          item.serviceName,
          item.taskPath
        );
      } else {
        await enableStartupItem(
          item.id,
          item.source,
          item.registryKey,
          item.serviceName,
          item.taskPath,
          item.commandLine
        );
      }

      // Optimistically flip the state in memory
      set((state) => ({
        items: state.items.map((i) =>
          i.id === item.id ? { ...i, isEnabled: !isCurrentlyEnabled } : i
        ),
        actionInProgressId: null,
      }));

      // Re-fetch boot summary in background to recalibrate
      void getBootPerformanceSummary()
        .then((s) => set({ summary: s }))
        .catch(() => {});

      return true;
    } catch (err) {
      set({
        actionInProgressId: null,
        error: err instanceof Error ? err.message : String(err),
      });
      return false;
    }
  },

  refreshWprStatus: async () => {
    try {
      const status = await getWprStatus();
      set({ wprStatus: status });
    } catch {
      // Ignored
    }
  },

  startBootTrace: async () => {
    const msg = await startWprBootTrace();
    void get().refreshWprStatus();
    return msg;
  },

  cancelBootTrace: async () => {
    const msg = await cancelWprBootTrace();
    void get().refreshWprStatus();
    return msg;
  },

  setSearchQuery: (query) => set({ searchQuery: query }),
  setFilterImpact: (impact) => set({ filterImpact: impact }),
  setFilterRecommendation: (rec) => set({ filterRecommendation: rec }),
  setFilterRunning: (running) => set({ filterRunning: running }),
  setFilterEnabled: (enabled) => set({ filterEnabled: enabled }),
  resetFilters: () =>
    set({
      searchQuery: "",
      filterImpact: "all",
      filterRecommendation: "all",
      filterRunning: "all",
      filterEnabled: "all",
    }),
}));
