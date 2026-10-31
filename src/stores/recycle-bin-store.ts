import { create } from "zustand";
import type { RecycleBinListing, RecycleBinSortField } from "../types/recycle-bin";
import { deleteRecycleBinItems, listRecycleBin, openRecycleBin } from "../services/recycle-bin";
import { formatStorageBytes, getStorageDrives } from "../services/storage";
import { useStorageStore } from "./storage-store";

export type RecycleBinPhase = "idle" | "loading" | "ready" | "deleting" | "error";

/** Shown after deleting, until dismissed. */
export interface RecycleBinNotice {
  tone: "ok" | "warn";
  message: string;
  /** Items that could not be deleted, with the reason. */
  failures: { name: string; reason: string }[];
}

interface RecycleBinStoreState {
  phase: RecycleBinPhase;
  listing: RecycleBinListing | null;
  error: string | null;
  notice: RecycleBinNotice | null;
  confirmDelete: boolean;
  /** Paths (inside $Recycle.Bin) of the ticked items. */
  selected: Set<string>;
  sortField: RecycleBinSortField;
  searchQuery: string;

  load: () => Promise<void>;
  toggleSelected: (path: string) => void;
  /** Ticks every given path, or unticks them all when they are all ticked already. */
  toggleAll: (paths: string[]) => void;
  clearSelection: () => void;
  askDeleteSelected: () => void;
  cancelDelete: () => void;
  confirmDeleteSelected: () => Promise<void>;
  openInExplorer: () => Promise<void>;
  setSortField: (field: RecycleBinSortField) => void;
  setSearchQuery: (query: string) => void;
  dismissNotice: () => void;
}

function messageOf(err: unknown, fallback: string): string {
  return typeof err === "string" ? err : err instanceof Error ? err.message : fallback;
}

/** Drops ticks for items that are no longer in the bin. */
function pruneSelection(selected: Set<string>, listing: RecycleBinListing): Set<string> {
  const present = new Set(listing.items.map((i) => i.path));
  return new Set([...selected].filter((p) => present.has(p)));
}

function refreshDrives() {
  // Free space changed, so refresh the drive cards
  void getStorageDrives()
    .then((drives) => useStorageStore.setState({ drives }))
    .catch(() => undefined);
}

export const useRecycleBinStore = create<RecycleBinStoreState>((set, get) => ({
  phase: "idle",
  listing: null,
  error: null,
  notice: null,
  confirmDelete: false,
  selected: new Set(),
  sortField: "deleted",
  searchQuery: "",

  load: async () => {
    const { phase } = get();
    if (phase === "loading" || phase === "deleting") return;
    set({ phase: "loading", error: null });
    try {
      const listing = await listRecycleBin();
      set({ listing, phase: "ready", selected: pruneSelection(get().selected, listing) });
    } catch (err) {
      set({ phase: "error", error: messageOf(err, "Could not read the Recycle Bin.") });
    }
  },

  toggleSelected: (path) => {
    const selected = new Set(get().selected);
    if (selected.has(path)) selected.delete(path);
    else selected.add(path);
    set({ selected });
  },

  toggleAll: (paths) => {
    const current = get().selected;
    const allTicked = paths.length > 0 && paths.every((p) => current.has(p));
    const selected = new Set(current);
    for (const p of paths) {
      if (allTicked) selected.delete(p);
      else selected.add(p);
    }
    set({ selected });
  },

  clearSelection: () => set({ selected: new Set() }),

  askDeleteSelected: () => {
    if (get().selected.size > 0) set({ confirmDelete: true });
  },
  cancelDelete: () => set({ confirmDelete: false }),

  confirmDeleteSelected: async () => {
    const paths = [...get().selected];
    const names = new Map((get().listing?.items ?? []).map((i) => [i.path, i.name]));
    set({ confirmDelete: false, phase: "deleting", error: null, notice: null });
    try {
      const report = await deleteRecycleBinItems(paths);
      const failures = report.failed.map((f) => ({ name: names.get(f.path) ?? f.path, reason: f.reason }));
      const deletedText = `${report.deleted.toLocaleString()} ${report.deleted === 1 ? "item" : "items"} deleted forever. ${formatStorageBytes(report.freedBytes)} freed.`;
      set({
        listing: report.listing,
        phase: "ready",
        selected: pruneSelection(get().selected, report.listing),
        notice: {
          tone: failures.length > 0 ? "warn" : "ok",
          message:
            failures.length > 0
              ? `${deletedText} ${failures.length.toLocaleString()} could not be deleted.`
              : deletedText,
          failures,
        },
      });
    } catch (err) {
      set({ phase: "ready", error: messageOf(err, "Could not delete the selected items.") });
      void get().load();
    }
    refreshDrives();
  },

  openInExplorer: async () => {
    try {
      await openRecycleBin();
    } catch (err) {
      set({ error: messageOf(err, "Could not open the Recycle Bin.") });
    }
  },

  setSortField: (field) => set({ sortField: field }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  dismissNotice: () => set({ notice: null }),
}));
