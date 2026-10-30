import { create } from "zustand";
import type {
  AsaoSettings,
  WidgetHistorySample,
  WidgetSystemUpdate,
} from "../types/widget";
import {
  getSettings,
  getWidgetUpdate,
  getWidgetVisibility,
  subscribeToWidgetEvents,
  toggleWidget,
  updateSettings,
} from "../services/widget";
import { recommendationService } from "../services/recommendation";
import { applyTheme, getStoredTheme } from "../services/theme";

const MAX_WIDGET_HISTORY = 24;

const DEFAULT_SETTINGS: AsaoSettings = {
  startWithWindows: false,
  widgetEnabled: true,
  launchWidgetOnStartup: false,
  alwaysOnTop: true,
  showRecommendations: true,
  notifications: true,
  widgetPosition: "bottom-right",
  customX: null,
  customY: null,
  widgetOpacity: 80,
  theme: getStoredTheme(),
  residualMinAgeDays: 30,
};

interface WidgetStoreState {
  widgetVisible: boolean;
  settings: AsaoSettings;
  telemetry: WidgetSystemUpdate | null;
  history: WidgetHistorySample[];

  initializeWidget: () => Promise<() => void>;
  toggleWidgetVisibility: () => Promise<void>;
  updateSettingsPatch: (patch: Partial<AsaoSettings>) => Promise<void>;
}

export const useWidgetStore = create<WidgetStoreState>((set, get) => ({
  widgetVisible: false,
  settings: DEFAULT_SETTINGS,
  telemetry: null,
  history: [],

  initializeWidget: async () => {
    const pushUpdate = (update: WidgetSystemUpdate) => {
      const prev = get().history;
      const sample: WidgetHistorySample = {
        timestamp: update.timestamp || Date.now(),
        cpuUsage: update.cpuUsage,
        memoryUsage: update.memoryUsage,
      };
      const nextHistory =
        prev.length >= MAX_WIDGET_HISTORY
          ? [...prev.slice(prev.length - MAX_WIDGET_HISTORY + 1), sample]
          : [...prev, sample];

      // Run recommendation priority & rate-limited notification check
      recommendationService.evaluateRecommendation(update, get().settings);

      set({
        telemetry: update,
        history: nextHistory,
      });
    };

    try {
      const [initialSettings, initialUpdate, initialVisible] =
        await Promise.all([
          getSettings(),
          getWidgetUpdate(),
          getWidgetVisibility(),
        ]);
      if (initialSettings.theme) {
        applyTheme(initialSettings.theme);
      }
      set({ settings: initialSettings, widgetVisible: initialVisible });
      pushUpdate(initialUpdate);
    } catch {
      // Fallback if initialized before first snapshot
    }

    const cleanup = await subscribeToWidgetEvents({
      onSystemUpdate: (update) => {
        pushUpdate(update);
      },
      onSettingsUpdated: (newSettings) => {
        if (newSettings.theme) {
          applyTheme(newSettings.theme);
        }
        set({
          settings: newSettings,
          widgetVisible: newSettings.widgetEnabled ? get().widgetVisible : false,
        });
      },
      onVisibilityChanged: (visible) => {
        set({ widgetVisible: visible });
      },
    });

    return cleanup;
  },

  toggleWidgetVisibility: async () => {
    try {
      const nextVisible = await toggleWidget();
      set({ widgetVisible: nextVisible });
    } catch {
      // Ignore error
    }
  },

  updateSettingsPatch: async (patch) => {
    if (patch.theme) {
      applyTheme(patch.theme);
    }
    const merged: AsaoSettings = {
      ...get().settings,
      ...patch,
    };
    set({ settings: merged });
    try {
      const saved = await updateSettings(merged);
      set({ settings: saved });
    } catch {
      // Keep optimistic update
    }
  },
}));
