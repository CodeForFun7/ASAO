import { create } from "zustand";
import type {
  CleanReport,
  ResidualCandidate,
  ResidualGroup,
  ResidualScanProgress,
  ResidualScanResult,
} from "../types/residual";
import {
  cancelLeftoverScan,
  cleanLeftovers,
  getLastLeftoverResult,
  ignoreApp,
  ignoreFolder,
  scanLeftovers,
  subscribeToLeftoverEvents,
} from "../services/residual";

export type LeftoverPhase = "idle" | "scanning" | "ready" | "error" | "cleaning";

const pathKey = (p: string) => p.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();

// Sizing starts in the backend before the scan command returns, so the first size events
// can arrive before the result does. They wait here until the result for their scan lands.
const pendingSizes = new Map<number, Map<string, { sizeBytes: number; newestModifiedMs: number | null }>>();
let sizingDoneFor: number | null = null;

function applyPendingSizes(result: ResidualScanResult): ResidualScanResult {
  const pending = pendingSizes.get(result.scanId);
  pendingSizes.clear();
  if (!pending) return result;
  return {
    ...result,
    groups: result.groups.map((g) => ({
      ...g,
      folders: g.folders.map((f) => {
        const u = pending.get(pathKey(f.path));
        if (!u) return f;
        return {
          ...f,
          sizeBytes: u.sizeBytes,
          newestModifiedMs: u.newestModifiedMs ?? f.newestModifiedMs,
        };
      }),
    })),
  };
}

interface ResidualStoreState {
  phase: LeftoverPhase;
  result: ResidualScanResult | null;
  progress: ResidualScanProgress | null;
  sizing: boolean;
  error: string | null;
  /** Ticked folders, by path key. */
  selected: Record<string, boolean>;
  expandedGroups: Record<string, boolean>;
  showUncertain: boolean;
  /** On by default: the whole flow runs, but nothing is deleted. */
  dryRun: boolean;
  permanent: boolean;
  confirmOpen: boolean;
  report: CleanReport | null;

  initialize: () => Promise<() => void>;
  scan: () => Promise<void>;
  cancelScan: () => Promise<void>;
  toggleFolder: (path: string) => void;
  setGroupSelected: (folders: ResidualCandidate[], value: boolean) => void;
  toggleExpanded: (key: string) => void;
  setShowUncertain: (value: boolean) => void;
  setDryRun: (value: boolean) => void;
  setPermanent: (value: boolean) => void;
  openConfirm: () => void;
  closeConfirm: () => void;
  clean: () => Promise<void>;
  closeReport: () => void;
  ignoreFolder: (path: string) => Promise<void>;
  ignoreApp: (group: ResidualGroup) => Promise<void>;
}

/** Only High confidence items start ticked; everything else waits for the user. */
function initialSelection(result: ResidualScanResult): Record<string, boolean> {
  const sel: Record<string, boolean> = {};
  for (const g of result.groups) {
    for (const f of g.folders) {
      if (f.preselected && f.confidence === "high") sel[pathKey(f.path)] = true;
    }
  }
  return sel;
}

function updateFolder(
  result: ResidualScanResult,
  path: string,
  patch: Partial<ResidualCandidate>
): ResidualScanResult {
  const key = pathKey(path);
  return {
    ...result,
    groups: result.groups.map((g) => ({
      ...g,
      folders: g.folders.map((f) => (pathKey(f.path) === key ? { ...f, ...patch } : f)),
    })),
  };
}

function withoutPaths(result: ResidualScanResult, paths: string[]): ResidualScanResult {
  const keys = new Set(paths.map(pathKey));
  return {
    ...result,
    groups: result.groups
      .map((g) => ({ ...g, folders: g.folders.filter((f) => !keys.has(pathKey(f.path))) }))
      .filter((g) => g.folders.length > 0),
  };
}

export const selectedCandidates = (s: Pick<ResidualStoreState, "result" | "selected">): ResidualCandidate[] =>
  (s.result?.groups ?? []).flatMap((g) => g.folders.filter((f) => s.selected[pathKey(f.path)]));

export const isSelected = (s: Pick<ResidualStoreState, "selected">, path: string) =>
  Boolean(s.selected[pathKey(path)]);

export const useResidualStore = create<ResidualStoreState>((set, get) => ({
  phase: "idle",
  result: null,
  progress: null,
  sizing: false,
  error: null,
  selected: {},
  expandedGroups: {},
  showUncertain: false,
  dryRun: true,
  permanent: false,
  confirmOpen: false,
  report: null,

  initialize: async () => {
    const unlisten = await subscribeToLeftoverEvents({
      onProgress: (p) => {
        if (get().phase === "scanning") set({ progress: p });
      },
      onSize: (u) => {
        const result = get().result;
        if (!result || result.scanId !== u.scanId) {
          const forScan = pendingSizes.get(u.scanId) ?? new Map();
          forScan.set(pathKey(u.path), { sizeBytes: u.sizeBytes, newestModifiedMs: u.newestModifiedMs });
          pendingSizes.set(u.scanId, forScan);
          return;
        }
        const patch: Partial<ResidualCandidate> = { sizeBytes: u.sizeBytes };
        if (u.newestModifiedMs !== null) patch.newestModifiedMs = u.newestModifiedMs;
        set({ result: updateFolder(result, u.path, patch) });
      },
      onSizingDone: (d) => {
        sizingDoneFor = d.scanId;
        if (get().result?.scanId !== d.scanId) return;
        set({ sizing: false });
        // The backend copy has every size; take it so no row is left at "Measuring".
        void getLastLeftoverResult()
          .then((latest) => {
            const current = get().result;
            if (latest && current && latest.scanId === current.scanId) {
              const keep = new Set(current.groups.flatMap((g) => g.folders.map((f) => pathKey(f.path))));
              set({
                result: {
                  ...latest,
                  groups: latest.groups
                    .map((g) => ({ ...g, folders: g.folders.filter((f) => keep.has(pathKey(f.path))) }))
                    .filter((g) => g.folders.length > 0),
                },
              });
            }
          })
          .catch(() => {});
      },
    });

    // Show the latest result (from a background or earlier scan) without rescanning.
    if (!get().result && get().phase === "idle") {
      try {
        const last = await getLastLeftoverResult();
        if (last && get().phase === "idle") {
          const unsized = last.groups.some((g) => g.folders.some((f) => f.sizeBytes === null));
          set({
            result: last,
            phase: "ready",
            sizing: unsized,
            selected: initialSelection(last),
          });
        }
      } catch {
        // Outside Tauri
      }
    }
    return unlisten;
  },

  scan: async () => {
    if (get().phase === "scanning" || get().phase === "cleaning") return;
    set({ phase: "scanning", progress: null, error: null, report: null });
    try {
      const scanned = await scanLeftovers();
      const result = applyPendingSizes(scanned);
      const alreadySized = sizingDoneFor === result.scanId;
      set({
        result,
        phase: "ready",
        sizing: !alreadySized,
        selected: initialSelection(result),
        expandedGroups: {},
      });
      if (alreadySized) {
        void getLastLeftoverResult()
          .then((latest) => {
            if (latest && latest.scanId === result.scanId) set({ result: latest });
          })
          .catch(() => {});
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      const cancelled = message === "Scan cancelled.";
      set({
        phase: get().result ? "ready" : cancelled ? "idle" : "error",
        error: cancelled ? null : message,
      });
    }
  },

  cancelScan: async () => {
    try {
      await cancelLeftoverScan();
    } catch {
      // ignore
    }
  },

  toggleFolder: (path) =>
    set((s) => {
      const key = pathKey(path);
      return { selected: { ...s.selected, [key]: !s.selected[key] } };
    }),

  setGroupSelected: (folders, value) =>
    set((s) => {
      const next = { ...s.selected };
      for (const f of folders) next[pathKey(f.path)] = value;
      return { selected: next };
    }),

  toggleExpanded: (key) =>
    set((s) => ({ expandedGroups: { ...s.expandedGroups, [key]: !s.expandedGroups[key] } })),

  setShowUncertain: (value) => set({ showUncertain: value }),
  setDryRun: (value) => set({ dryRun: value }),
  setPermanent: (value) => set({ permanent: value }),
  openConfirm: () => {
    if (selectedCandidates(get()).length > 0) set({ confirmOpen: true });
  },
  closeConfirm: () => set({ confirmOpen: false }),

  clean: async () => {
    const items = selectedCandidates(get());
    if (items.length === 0) return;
    const { dryRun, permanent } = get();
    set({ phase: "cleaning", confirmOpen: false, error: null });
    try {
      const report = await cleanLeftovers(
        items.map((i) => i.path),
        dryRun,
        permanent
      );
      const deleted = report.outcomes.filter((o) => o.status === "deleted").map((o) => o.path);
      const result = get().result;
      const selected = { ...get().selected };
      for (const p of deleted) delete selected[pathKey(p)];
      set({
        report,
        phase: "ready",
        result: result ? withoutPaths(result, deleted) : result,
        selected,
      });
    } catch (err) {
      set({ phase: "ready", error: err instanceof Error ? err.message : String(err) });
    }
  },

  closeReport: () => set({ report: null }),

  ignoreFolder: async (path) => {
    try {
      await ignoreFolder(path);
      const result = get().result;
      const selected = { ...get().selected };
      delete selected[pathKey(path)];
      set({ result: result ? withoutPaths(result, [path]) : result, selected });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
    }
  },

  ignoreApp: async (group) => {
    if (group.isUnknown) return;
    try {
      await ignoreApp(group.key, group.appName);
      const result = get().result;
      const selected = { ...get().selected };
      for (const f of group.folders) delete selected[pathKey(f.path)];
      set({
        result: result
          ? { ...result, groups: result.groups.filter((g) => g.isUnknown || g.key !== group.key) }
          : result,
        selected,
      });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
    }
  },
}));
