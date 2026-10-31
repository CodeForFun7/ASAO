import { create } from "zustand";
import type {
  BloatApplyProgress,
  BloatApplyReport,
  BloatCategory,
  BloatInfo,
  BloatItem,
  BloatPlan,
  BloatSafety,
  BloatScanProgress,
  BloatScanResult,
  HistoryBatch,
} from "../types/bloat";
import {
  applyBloat,
  cancelBloatScan,
  getBloatHistory,
  getBloatInfo,
  getLastBloatResult,
  planBloat,
  scanBloat,
  subscribeToApplyProgress,
  subscribeToBloatProgress,
  undoBloatChange,
} from "../services/bloat";

export type BloatPhase = "idle" | "scanning" | "ready" | "error" | "applying";
export type BloatTab = "recommended" | "apps" | "services" | "tweaks" | "recent";

interface BloatStoreState {
  phase: BloatPhase;
  result: BloatScanResult | null;
  progress: BloatScanProgress | null;
  error: string | null;
  /** Ticked items, by id. Only selectable items can be in here. */
  selected: Record<string, boolean>;
  /** Ticked tweaks (not in a group), by feature id. */
  selectedTweaks: Record<string, boolean>;
  /** Chosen option per pick-one group, when it differs from the current one. */
  groupChoices: Record<string, string>;
  tab: BloatTab;
  safetyFilter: BloatSafety | "all";
  categoryFilter: BloatCategory | "all";
  detailId: string | null;

  info: BloatInfo | null;
  /** Development builds: on by default. Release: off, unless forced by config or flag. */
  dryRun: boolean;
  confirmOpen: boolean;
  plan: BloatPlan | null;
  removeProvisioned: boolean;
  continueWithoutRestorePoint: boolean;
  applyProgress: BloatApplyProgress | null;
  report: BloatApplyReport | null;
  history: HistoryBatch[];
  undoing: Record<string, boolean>;
  undoMessages: Record<string, { ok: boolean; message: string }>;

  initialize: () => Promise<() => void>;
  scan: () => Promise<void>;
  cancelScan: () => Promise<void>;
  toggle: (item: BloatItem) => void;
  toggleTweak: (id: string) => void;
  chooseGroupOption: (groupId: string, featureId: string | null) => void;
  applyPreset: (name: string) => void;
  setTab: (tab: BloatTab) => void;
  setSafetyFilter: (v: BloatSafety | "all") => void;
  setCategoryFilter: (v: BloatCategory | "all") => void;
  openDetail: (id: string | null) => void;
  setDryRun: (v: boolean) => void;
  openConfirm: () => Promise<void>;
  closeConfirm: () => void;
  setRemoveProvisioned: (v: boolean) => Promise<void>;
  setContinueWithoutRestorePoint: (v: boolean) => void;
  apply: () => Promise<void>;
  closeReport: () => void;
  loadHistory: () => Promise<void>;
  undo: (batchId: string, actionId: string) => Promise<void>;
}

function initialSelection(result: BloatScanResult): Record<string, boolean> {
  const sel: Record<string, boolean> = {};
  for (const i of result.items) if (i.selectable && i.preselected) sel[i.id] = true;
  return sel;
}

/** Win11Debloat's default tweaks that are not in effect yet. */
function initialTweaks(result: BloatScanResult): Record<string, boolean> {
  const sel: Record<string, boolean> = {};
  for (const t of result.tweaks?.tweaks ?? []) if (t.preselected && !t.groupId && t.applied !== true) sel[t.id] = true;
  return sel;
}

/** Feature ids to apply: ticked tweaks plus chosen group options. */
export const selectedTweakIds = (s: Pick<BloatStoreState, "selectedTweaks" | "groupChoices">): string[] => [
  ...Object.keys(s.selectedTweaks).filter((k) => s.selectedTweaks[k]),
  ...Object.values(s.groupChoices),
];

/** Win11Debloat removes every package whose name contains the preset's id. */
export function inPreset(item: BloatItem, appIds: string[]): boolean {
  const pkg = item.packageName?.toLowerCase();
  return Boolean(pkg && appIds.some((id) => pkg.includes(id.toLowerCase())));
}

/** Items ASAO recommends removing: selectable, and the database says Remove. */
export const isRecommended = (i: BloatItem) => i.selectable && i.recommendation === "remove";

export const selectedItems = (s: Pick<BloatStoreState, "result" | "selected">): BloatItem[] =>
  (s.result?.items ?? []).filter((i) => i.selectable && s.selected[i.id]);

const message = (err: unknown) => (err instanceof Error ? err.message : String(err));

export const useBloatStore = create<BloatStoreState>((set, get) => ({
  phase: "idle",
  result: null,
  progress: null,
  error: null,
  selected: {},
  selectedTweaks: {},
  groupChoices: {},
  tab: "recommended",
  safetyFilter: "all",
  categoryFilter: "all",
  detailId: null,
  info: null,
  dryRun: true,
  confirmOpen: false,
  plan: null,
  removeProvisioned: false,
  continueWithoutRestorePoint: false,
  applyProgress: null,
  report: null,
  history: [],
  undoing: {},
  undoMessages: {},

  initialize: async () => {
    const unlisteners: (() => void)[] = [];
    try {
      unlisteners.push(
        await subscribeToBloatProgress((p) => {
          if (get().phase === "scanning") set({ progress: p });
        })
      );
      unlisteners.push(
        await subscribeToApplyProgress((p) => {
          if (get().phase === "applying") set({ applyProgress: p });
        })
      );
    } catch {
      // Outside Tauri
    }
    try {
      const info = await getBloatInfo();
      // Only the first load decides the default; the user's switch is kept after that.
      if (!get().info) set({ info, dryRun: info.dryRunDefault });
      else set({ info });
    } catch {
      // Outside Tauri
    }
    if (!get().result && get().phase === "idle") {
      try {
        const last = await getLastBloatResult();
        if (last && get().phase === "idle") {
          set({
            result: last,
            phase: "ready",
            selected: initialSelection(last),
            selectedTweaks: initialTweaks(last),
            groupChoices: {},
          });
        }
      } catch {
        // Outside Tauri
      }
    }
    void get().loadHistory();
    return () => unlisteners.forEach((u) => u());
  },

  scan: async () => {
    if (get().phase === "scanning" || get().phase === "applying") return;
    set({ phase: "scanning", progress: null, error: null, detailId: null });
    try {
      const result = await scanBloat();
      set({
        result,
        phase: "ready",
        selected: initialSelection(result),
        selectedTweaks: initialTweaks(result),
        groupChoices: {},
      });
    } catch (err) {
      const m = message(err);
      const cancelled = m === "Scan cancelled.";
      set({
        phase: get().result ? "ready" : cancelled ? "idle" : "error",
        error: cancelled ? null : m,
      });
    }
  },

  cancelScan: async () => {
    try {
      await cancelBloatScan();
    } catch {
      // Outside Tauri
    }
  },

  toggle: (item) => {
    if (!item.selectable) return;
    const selected = { ...get().selected };
    if (selected[item.id]) delete selected[item.id];
    else selected[item.id] = true;
    set({ selected });
  },

  toggleTweak: (id) => {
    const t = get().result?.tweaks.tweaks.find((x) => x.id === id);
    if (!t || t.applied === true || t.groupId) return;
    const selectedTweaks = { ...get().selectedTweaks };
    if (selectedTweaks[id]) delete selectedTweaks[id];
    else selectedTweaks[id] = true;
    set({ selectedTweaks });
  },

  chooseGroupOption: (groupId, featureId) => {
    const g = get().result?.tweaks.groups.find((x) => x.id === groupId);
    const groupChoices = { ...get().groupChoices };
    if (!featureId || !g || g.current === featureId) delete groupChoices[groupId];
    else groupChoices[groupId] = featureId;
    set({ groupChoices });
  },

  applyPreset: (name) => {
    const result = get().result;
    const preset = result?.presets.find((p) => p.name === name);
    if (!result || !preset) return;
    const selected = { ...get().selected };
    for (const i of result.items) if (i.selectable && inPreset(i, preset.appIds)) selected[i.id] = true;
    set({ selected });
  },

  setTab: (tab) => {
    set({ tab });
    if (tab === "recent") void get().loadHistory();
  },
  setSafetyFilter: (safetyFilter) => set({ safetyFilter }),
  setCategoryFilter: (categoryFilter) => set({ categoryFilter }),
  openDetail: (detailId) => set({ detailId }),

  setDryRun: (dryRun) => {
    // The switch only exists in development builds; a forced dry run cannot be turned off.
    if (get().info?.dryRunForced) return;
    set({ dryRun });
  },

  openConfirm: async () => {
    const ids = selectedItems(get()).map((i) => i.id);
    const tweaks = selectedTweakIds(get());
    if (ids.length === 0 && tweaks.length === 0) return;
    try {
      const plan = await planBloat(ids, tweaks, get().removeProvisioned);
      set({ plan, confirmOpen: true, continueWithoutRestorePoint: false, error: null });
    } catch (err) {
      set({ error: message(err) });
    }
  },

  closeConfirm: () => set({ confirmOpen: false }),

  setRemoveProvisioned: async (removeProvisioned) => {
    set({ removeProvisioned });
    const ids = selectedItems(get()).map((i) => i.id);
    try {
      set({ plan: await planBloat(ids, selectedTweakIds(get()), removeProvisioned) });
    } catch (err) {
      set({ error: message(err) });
    }
  },

  setContinueWithoutRestorePoint: (continueWithoutRestorePoint) => set({ continueWithoutRestorePoint }),

  apply: async () => {
    const ids = selectedItems(get()).map((i) => i.id);
    const tweaks = selectedTweakIds(get());
    if (ids.length === 0 && tweaks.length === 0) return;
    const { dryRun, removeProvisioned, continueWithoutRestorePoint } = get();
    set({ phase: "applying", confirmOpen: false, applyProgress: null, error: null });
    try {
      const report = await applyBloat(ids, tweaks, dryRun, removeProvisioned, continueWithoutRestorePoint);
      const result = get().result;
      const gone = new Set(
        report.outcomes
          .filter((o) => o.status === "done")
          .filter((o) => o.action.kind.type === "uninstall-store" || o.action.kind.type === "uninstall-win32")
          .map((o) => o.action.itemId)
      );
      const selected = { ...get().selected };
      for (const id of gone) delete selected[id];
      set({
        report,
        phase: "ready",
        selected,
        result: result ? { ...result, items: result.items.filter((i) => !gone.has(i.id)) } : result,
      });
      void get().loadHistory();
    } catch (err) {
      set({ phase: "ready", error: message(err) });
    }
  },

  closeReport: () => {
    const report = get().report;
    set({ report: null });
    // After real changes, scan again so every row shows its new state.
    if (report && !report.dryRun && report.outcomes.some((o) => o.status === "done")) void get().scan();
  },

  loadHistory: async () => {
    try {
      const h = await getBloatHistory();
      set({ history: h.batches });
    } catch {
      // Outside Tauri
    }
  },

  undo: async (batchId, actionId) => {
    const key = `${batchId}/${actionId}`;
    set({ undoing: { ...get().undoing, [key]: true } });
    try {
      const r = await undoBloatChange(batchId, actionId);
      set({ undoMessages: { ...get().undoMessages, [key]: r } });
    } catch (err) {
      set({ undoMessages: { ...get().undoMessages, [key]: { ok: false, message: message(err) } } });
    } finally {
      const undoing = { ...get().undoing };
      delete undoing[key];
      set({ undoing });
      void get().loadHistory();
    }
  },
}));
