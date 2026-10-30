import { create } from "zustand";
import type { ActionOutcome, RemovalMethod, UnusedApp, UnusedAppsResult } from "../types/apps";
import {
  getLastUnusedApps,
  ignoreUnusedApp,
  removeApp,
  scanUnusedApps,
  subscribeToAppEvents,
} from "../services/apps";

export type AppsPhase = "idle" | "scanning" | "ready" | "error";

/** Where a removal stands, per app id. */
export interface PendingAction {
  status: "working" | "waiting" | "removed" | "still-installed" | "done" | "declined" | "failed";
  message: string;
}

interface ConfirmState {
  app: UnusedApp;
  method: RemovalMethod;
  useFallback: boolean;
}

interface AppsStoreState {
  phase: AppsPhase;
  result: UnusedAppsResult | null;
  error: string | null;
  /** Show apps not used for at least this many days. */
  thresholdDays: number;
  showNoRecord: boolean;
  confirm: ConfirmState | null;
  pending: Record<string, PendingAction>;

  initialize: () => Promise<() => void>;
  scan: () => Promise<void>;
  setThreshold: (days: number) => void;
  setShowNoRecord: (value: boolean) => void;
  askRemove: (app: UnusedApp, useFallback: boolean) => void;
  cancelRemove: () => void;
  confirmRemove: () => Promise<void>;
  ignore: (app: UnusedApp) => Promise<void>;
  dismissPending: (id: string) => void;
}

function statusFrom(outcome: ActionOutcome): PendingAction {
  switch (outcome.status) {
    case "launched":
      return { status: outcome.watching ? "waiting" : "done", message: outcome.message };
    case "done":
      return { status: "removed", message: outcome.message };
    case "declined":
      return { status: "declined", message: outcome.message };
    default:
      return { status: "failed", message: outcome.message };
  }
}

export const useAppsStore = create<AppsStoreState>((set, get) => ({
  phase: "idle",
  result: null,
  error: null,
  thresholdDays: 90,
  showNoRecord: false,
  confirm: null,
  pending: {},

  initialize: async () => {
    const unlisten = await subscribeToAppEvents({
      onSize: (u) => {
        const r = get().result;
        if (!r || r.scanId !== u.scanId) return;
        set({
          result: { ...r, apps: r.apps.map((a) => (a.id === u.id ? { ...a, sizeBytes: u.sizeBytes } : a)) },
        });
      },
      onFinished: (f) => {
        set({
          pending: {
            ...get().pending,
            [f.id]: f.removed
              ? {
                  status: "removed",
                  message: `${f.name} was uninstalled. Any folders it left behind now show on the Leftovers tab.`,
                }
              : { status: "still-installed", message: `${f.name} is still installed.` },
          },
        });
      },
    });
    if (!get().result && get().phase === "idle") {
      try {
        const last = await getLastUnusedApps();
        if (last && get().phase === "idle") set({ result: last, phase: "ready" });
      } catch {
        // Outside Tauri
      }
    }
    return unlisten;
  },

  scan: async () => {
    if (get().phase === "scanning") return;
    set({ phase: "scanning", error: null });
    try {
      const result = await scanUnusedApps();
      // Keep finished or waiting notes only for apps that are still listed
      const ids = new Set(result.apps.map((a) => a.id));
      const pending = Object.fromEntries(
        Object.entries(get().pending).filter(([id, p]) => ids.has(id) && p.status === "waiting")
      );
      set({ result, phase: "ready", pending });
    } catch (err) {
      set({ phase: get().result ? "ready" : "error", error: err instanceof Error ? err.message : String(err) });
    }
  },

  setThreshold: (days) => set({ thresholdDays: days }),
  setShowNoRecord: (value) => set({ showNoRecord: value }),

  askRemove: (app, useFallback) => {
    const method = useFallback ? app.fallback : app.method;
    if (method) set({ confirm: { app, method, useFallback } });
  },
  cancelRemove: () => set({ confirm: null }),

  confirmRemove: async () => {
    const c = get().confirm;
    if (!c) return;
    set({
      confirm: null,
      pending: { ...get().pending, [c.app.id]: { status: "working", message: "Starting..." } },
    });
    try {
      const outcome = await removeApp(c.app.id, c.useFallback);
      set({ pending: { ...get().pending, [c.app.id]: statusFrom(outcome) } });
    } catch (err) {
      set({
        pending: {
          ...get().pending,
          [c.app.id]: { status: "failed", message: err instanceof Error ? err.message : String(err) },
        },
      });
    }
  },

  ignore: async (app) => {
    try {
      await ignoreUnusedApp(app.key, app.name);
      const r = get().result;
      if (r) set({ result: { ...r, apps: r.apps.filter((a) => a.key !== app.key) } });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
    }
  },

  dismissPending: (id) => {
    const next = { ...get().pending };
    delete next[id];
    set({ pending: next });
  },
}));
