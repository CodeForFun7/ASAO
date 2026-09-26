import { create } from "zustand";
import type {
  AsaoSettings,
  ChatMessage,
  WidgetHistorySample,
  WidgetMode,
  WidgetSystemUpdate,
} from "../types/widget";
import {
  getSettings,
  getWidgetUpdate,
  getWidgetVisibility,
  setWidgetMode,
  subscribeToWidgetEvents,
  toggleWidget,
  updateSettings,
} from "../services/widget";
import { chatService, systemContextService } from "../services/chat";
import { recommendationService } from "../services/recommendation";

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
};

interface WidgetStoreState {
  mode: WidgetMode;
  widgetVisible: boolean;
  settings: AsaoSettings;
  telemetry: WidgetSystemUpdate | null;
  history: WidgetHistorySample[];
  messages: ChatMessage[];
  isSendingChat: boolean;

  initializeWidget: () => Promise<() => void>;
  toggleWidgetVisibility: () => Promise<void>;
  switchMode: (mode: WidgetMode, initialQuestion?: string) => Promise<void>;
  updateSettingsPatch: (patch: Partial<AsaoSettings>) => Promise<void>;
  sendChatPrompt: (promptText: string) => Promise<void>;
}

export const useWidgetStore = create<WidgetStoreState>((set, get) => ({
  mode: "monitor",
  widgetVisible: false,
  settings: DEFAULT_SETTINGS,
  telemetry: null,
  history: [],
  messages: [
    {
      id: "welcome-msg",
      sender: "asao",
      text: "Monitoring your system in real time. Ask me about running processes, memory usage, or current recommendations.",
      timestamp: Date.now(),
    },
  ],
  isSendingChat: false,

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

  switchMode: async (mode, initialQuestion) => {
    set({ mode });
    try {
      await setWidgetMode(mode);
    } catch {
      // Ignore window resize error in browser preview
    }
    if (mode === "chat" && initialQuestion) {
      await get().sendChatPrompt(initialQuestion);
    }
  },

  updateSettingsPatch: async (patch) => {
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

  sendChatPrompt: async (promptText) => {
    const trimmed = promptText.trim();
    if (!trimmed || get().isSendingChat) return;

    const userMsg: ChatMessage = {
      id: `user-${Date.now()}`,
      sender: "user",
      text: trimmed,
      timestamp: Date.now(),
    };

    set((state) => ({
      messages: [...state.messages, userMsg],
      isSendingChat: true,
    }));

    try {
      const telemetry = get().telemetry;
      if (telemetry) {
        const ctx = systemContextService.buildContext(
          telemetry,
          get().history
        );
        const reply = await chatService.sendMessage(trimmed, ctx);
        set((state) => ({
          messages: [...state.messages, reply],
          isSendingChat: false,
        }));
      } else {
        set((state) => ({
          messages: [
            ...state.messages,
            {
              id: `asao-${Date.now()}`,
              sender: "asao",
              text: "AI analysis will be available once the AI service is connected.",
              timestamp: Date.now(),
            },
          ],
          isSendingChat: false,
        }));
      }
    } catch {
      set({ isSendingChat: false });
    }
  },
}));
