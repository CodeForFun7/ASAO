import React, { useEffect, useMemo } from "react";
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  ExternalLink,
  File,
  Folder,
  Loader2,
  Minus,
  RefreshCw,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { useRecycleBinStore } from "../../../stores/recycle-bin-store";
import type { RecycleBinItem, RecycleBinSortField } from "../../../types/recycle-bin";
import { formatStorageBytes, formatStorageTimestampDetailed } from "../../../services/storage";

/** Rendering thousands of rows makes the page sluggish. The rest are still counted. */
const MAX_ROWS = 500;

const SORTS: { field: RecycleBinSortField; label: string }[] = [
  { field: "deleted", label: "Date deleted" },
  { field: "size", label: "Size" },
  { field: "name", label: "Name" },
];

function sortItems(items: RecycleBinItem[], field: RecycleBinSortField): RecycleBinItem[] {
  const sorted = [...items];
  switch (field) {
    case "size":
      return sorted.sort((a, b) => (b.sizeBytes ?? 0) - (a.sizeBytes ?? 0));
    case "name":
      return sorted.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
    default:
      return sorted.sort((a, b) => (b.deletedMs ?? 0) - (a.deletedMs ?? 0));
  }
}

const Checkbox: React.FC<{
  checked: boolean;
  /** Some, not all, are ticked. */
  partial?: boolean;
  disabled?: boolean;
  label: string;
  onChange: () => void;
}> = ({ checked, partial = false, disabled = false, label, onChange }) => (
  <button
    type="button"
    role="checkbox"
    aria-checked={partial ? "mixed" : checked}
    aria-label={label}
    title={label}
    disabled={disabled}
    onClick={(e) => {
      e.stopPropagation();
      onChange();
    }}
    className={`w-4 h-4 rounded border flex items-center justify-center shrink-0 transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed ${
      checked || partial
        ? "bg-lunar-white border-lunar-white text-lunar-bg"
        : "bg-lunar-bg border-lunar-border hover:border-lunar-text-sec"
    }`}
  >
    {partial ? <Minus className="w-2.5 h-2.5 stroke-[3]" /> : checked ? <Check className="w-2.5 h-2.5 stroke-[3]" /> : null}
  </button>
);

export const RecycleBinView: React.FC = () => {
  const phase = useRecycleBinStore((s) => s.phase);
  const listing = useRecycleBinStore((s) => s.listing);
  const error = useRecycleBinStore((s) => s.error);
  const notice = useRecycleBinStore((s) => s.notice);
  const confirmDelete = useRecycleBinStore((s) => s.confirmDelete);
  const selected = useRecycleBinStore((s) => s.selected);
  const toggleSelected = useRecycleBinStore((s) => s.toggleSelected);
  const toggleAll = useRecycleBinStore((s) => s.toggleAll);
  const clearSelection = useRecycleBinStore((s) => s.clearSelection);
  const askDeleteSelected = useRecycleBinStore((s) => s.askDeleteSelected);
  const cancelDelete = useRecycleBinStore((s) => s.cancelDelete);
  const confirmDeleteSelected = useRecycleBinStore((s) => s.confirmDeleteSelected);
  const sortField = useRecycleBinStore((s) => s.sortField);
  const searchQuery = useRecycleBinStore((s) => s.searchQuery);
  const load = useRecycleBinStore((s) => s.load);
  const openInExplorer = useRecycleBinStore((s) => s.openInExplorer);
  const setSortField = useRecycleBinStore((s) => s.setSortField);
  const setSearchQuery = useRecycleBinStore((s) => s.setSearchQuery);
  const dismissNotice = useRecycleBinStore((s) => s.dismissNotice);

  // Re-read the bin every time the tab opens: other apps add to it all the time.
  useEffect(() => {
    void load();
  }, [load]);

  const visible = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    const items = listing?.items ?? [];
    const filtered = q
      ? items.filter(
          (i) => i.name.toLowerCase().includes(q) || (i.originalFolder ?? "").toLowerCase().includes(q)
        )
      : items;
    return sortItems(filtered, sortField);
  }, [listing, searchQuery, sortField]);

  const selectedItems = useMemo(
    () => (listing?.items ?? []).filter((i) => selected.has(i.path)),
    [listing, selected]
  );
  const selectedBytes = selectedItems.reduce((acc, i) => acc + (i.sizeBytes ?? 0), 0);
  const visiblePaths = useMemo(() => visible.map((i) => i.path), [visible]);
  const visibleTicked = visiblePaths.filter((p) => selected.has(p)).length;

  const busy = phase === "loading" || phase === "deleting";
  const isEmpty = listing !== null && listing.totalItems === 0;

  return (
    <div className="space-y-5">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-lunar-white tracking-tight">Recycle Bin</h2>
          <p className="text-xs text-lunar-text-sec mt-0.5">
            Files you deleted on every drive. They still use disk space until the bin is emptied.
          </p>
        </div>
        <div className="flex items-center gap-2 shrink-0">
          <button
            type="button"
            onClick={() => void openInExplorer()}
            title="Open the Recycle Bin in Explorer to restore items"
            className="inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border bg-lunar-surface hover:bg-lunar-elevated text-lunar-text-sec hover:text-lunar-white border-lunar-border transition-colors cursor-pointer"
          >
            <ExternalLink className="w-3.5 h-3.5" />
            <span>Open in Explorer</span>
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => void load()}
            className={`inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border transition-colors ${
              busy
                ? "bg-lunar-surface text-lunar-muted border-lunar-border cursor-not-allowed"
                : "bg-lunar-elevated hover:bg-lunar-border text-lunar-white border-lunar-border cursor-pointer"
            }`}
          >
            <RefreshCw className={`w-3.5 h-3.5 ${phase === "loading" ? "animate-spin" : ""}`} />
            <span>Refresh</span>
          </button>
        </div>
      </div>

      {error && phase !== "error" && (
        <div className="flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg bg-lunar-critical/10 border border-lunar-critical/30">
          <AlertTriangle className="w-3.5 h-3.5 text-lunar-critical shrink-0 mt-0.5" />
          <p className="text-[11px] text-lunar-text leading-relaxed">{error}</p>
        </div>
      )}

      {notice && (
        <div
          className={`flex items-start gap-2.5 px-3.5 py-2.5 rounded-lg border ${
            notice.tone === "ok" ? "bg-lunar-healthy/10 border-lunar-healthy/30" : "bg-lunar-warning/10 border-lunar-warning/30"
          }`}
        >
          {notice.tone === "ok" ? (
            <CheckCircle2 className="w-3.5 h-3.5 text-lunar-healthy shrink-0 mt-0.5" />
          ) : (
            <AlertTriangle className="w-3.5 h-3.5 text-lunar-warning shrink-0 mt-0.5" />
          )}
          <div className="flex-1 min-w-0">
            <p className="text-[11px] text-lunar-text leading-relaxed">{notice.message}</p>
            {notice.failures.length > 0 && (
              <ul className="mt-1.5 space-y-0.5">
                {notice.failures.slice(0, 8).map((f, idx) => (
                  <li key={idx} className="text-[10px] text-lunar-text-sec truncate" title={`${f.name}: ${f.reason}`}>
                    <span className="text-lunar-text">{f.name}</span>: {f.reason}
                  </li>
                ))}
                {notice.failures.length > 8 && (
                  <li className="text-[10px] text-lunar-muted">and {notice.failures.length - 8} more</li>
                )}
              </ul>
            )}
          </div>
          <button
            type="button"
            onClick={dismissNotice}
            aria-label="Dismiss"
            className="p-0.5 rounded text-lunar-muted hover:text-lunar-white transition-colors cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}

      {phase === "loading" && !listing && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <Loader2 className="w-7 h-7 text-lunar-text-sec mb-3 animate-spin" />
          <p className="text-sm font-medium text-lunar-white">Reading the Recycle Bin</p>
          <p className="text-xs text-lunar-text-sec mt-1">Measuring deleted folders can take a few seconds.</p>
        </div>
      )}

      {phase === "error" && (
        <div className="rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
          <AlertTriangle className="w-8 h-8 text-lunar-critical mb-3 stroke-[1.5]" />
          <p className="text-sm font-medium text-lunar-white">{error || "Could not read the Recycle Bin."}</p>
          <button
            type="button"
            onClick={() => void load()}
            className="mt-4 inline-flex items-center gap-2 px-4 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
          >
            <RefreshCw className="w-3.5 h-3.5" />
            <span>Try again</span>
          </button>
        </div>
      )}

      {listing && isEmpty && phase !== "error" && (
        // Fill the space below the headers so the card sits in the middle of the page
        <div className="min-h-[calc(100vh-20rem)] flex items-center justify-center">
          <div className="w-full max-w-md rounded-lg lunar-glass-card p-10 flex flex-col items-center justify-center text-center">
            <CheckCircle2 className="w-8 h-8 text-lunar-healthy mb-3 stroke-[1.5]" />
            <p className="text-sm font-medium text-lunar-white">The Recycle Bin is empty.</p>
          </div>
        </div>
      )}

      {listing && !isEmpty && phase !== "error" && (
        <>
          <div className="rounded-xl lunar-glass-card p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div>
              <div className="text-sm font-semibold text-lunar-white">
                {listing.totalItems.toLocaleString()} {listing.totalItems === 1 ? "item" : "items"},{" "}
                {formatStorageBytes(listing.totalBytes)}
              </div>
              <div className="text-[11px] text-lunar-text-sec mt-1">
                To get an item back, open the bin in Explorer and choose Restore.
              </div>
            </div>
            <div className="flex items-center gap-1 p-1 rounded-lg bg-lunar-surface border border-lunar-border shrink-0">
              {SORTS.map(({ field, label }) => (
                <button
                  key={field}
                  type="button"
                  onClick={() => setSortField(field)}
                  className={`px-2.5 py-1 rounded-md text-[11px] font-medium border transition-colors cursor-pointer ${
                    sortField === field
                      ? "bg-lunar-elevated text-lunar-white border-lunar-border"
                      : "text-lunar-text-sec hover:text-lunar-text border-transparent"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>

          <div className="relative">
            <Search className="w-3.5 h-3.5 text-lunar-muted absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search by name or original location"
              className="w-full pl-9 pr-3 py-2 rounded-lg bg-lunar-surface border border-lunar-border text-xs text-lunar-white placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec"
            />
          </div>

          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 px-4 py-2.5 rounded-xl bg-lunar-surface border border-lunar-border">
            <label className="flex items-center gap-2.5 cursor-pointer select-none">
              <Checkbox
                checked={visiblePaths.length > 0 && visibleTicked === visiblePaths.length}
                partial={visibleTicked > 0 && visibleTicked < visiblePaths.length}
                disabled={busy || visiblePaths.length === 0}
                label={searchQuery.trim() ? "Select all matching items" : "Select all"}
                onChange={() => toggleAll(visiblePaths)}
              />
              <span className="text-[11px] text-lunar-text">
                {searchQuery.trim() ? "Select all matching" : "Select all"} ({visiblePaths.length.toLocaleString()})
              </span>
            </label>
            <div className="flex items-center gap-2">
              {selected.size > 0 && (
                <>
                  <span className="text-[11px] text-lunar-text-sec">
                    {selected.size.toLocaleString()} selected, {formatStorageBytes(selectedBytes)}
                  </span>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={clearSelection}
                    className="px-2.5 py-1.5 rounded-lg text-[11px] font-medium text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
                  >
                    Clear
                  </button>
                </>
              )}
              <button
                type="button"
                disabled={busy || selected.size === 0}
                onClick={askDeleteSelected}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-medium border bg-lunar-critical/15 hover:bg-lunar-critical/25 text-lunar-white border-lunar-critical/40 transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
              >
                {phase === "deleting" ? <Loader2 className="w-3 h-3 animate-spin" /> : <Trash2 className="w-3 h-3" />}
                <span>{phase === "deleting" ? "Deleting" : "Delete selected forever"}</span>
              </button>
            </div>
          </div>

          {visible.length === 0 ? (
            <div className="rounded-lg lunar-glass-card p-8 text-center text-xs text-lunar-text-sec">
              Nothing matches "{searchQuery}".
            </div>
          ) : (
            <div className="space-y-2">
              {visible.slice(0, MAX_ROWS).map((item) => (
                <div
                  key={item.path}
                  onClick={() => !busy && toggleSelected(item.path)}
                  className={`px-4 py-3 rounded-xl border flex items-center gap-3 cursor-pointer transition-colors ${
                    selected.has(item.path)
                      ? "bg-lunar-elevated/70 border-lunar-text-sec/50"
                      : "bg-lunar-surface/70 border-lunar-border hover:border-lunar-text-sec/40"
                  }`}
                >
                  <Checkbox
                    checked={selected.has(item.path)}
                    disabled={busy}
                    label={`Select ${item.name}`}
                    onChange={() => toggleSelected(item.path)}
                  />
                  <div className="w-7 h-7 rounded-md bg-lunar-elevated border border-lunar-border flex items-center justify-center shrink-0">
                    {item.isFolder ? (
                      <Folder className="w-3.5 h-3.5 text-lunar-text-sec" />
                    ) : (
                      <File className="w-3.5 h-3.5 text-lunar-text-sec" />
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 min-w-0">
                      <span className="text-xs font-semibold text-lunar-white truncate" title={item.name}>
                        {item.name}
                      </span>
                      {item.typeName && (
                        <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-text-sec border border-lunar-border shrink-0 max-w-[12rem] truncate">
                          {item.typeName}
                        </span>
                      )}
                    </div>
                    <div className="text-[11px] text-lunar-text-sec truncate mt-0.5" title={item.originalFolder ?? undefined}>
                      {item.originalFolder ? `From ${item.originalFolder}` : "Original location unknown"}
                    </div>
                  </div>
                  <div className="w-40 shrink-0 text-right hidden md:block">
                    <div className="text-[10px] text-lunar-muted">Deleted</div>
                    <div className="text-[11px] text-lunar-text">{formatStorageTimestampDetailed(item.deletedMs)}</div>
                  </div>
                  <div className="w-20 shrink-0 text-right">
                    <div className="text-[10px] text-lunar-muted">Size</div>
                    <div className="text-xs font-mono font-semibold text-lunar-white">
                      {formatStorageBytes(item.sizeBytes)}
                    </div>
                  </div>
                </div>
              ))}
              {visible.length > MAX_ROWS && (
                <p className="text-[11px] text-lunar-text-sec text-center pt-1">
                  Showing {MAX_ROWS.toLocaleString()} of {visible.length.toLocaleString()} items. Search or sort to
                  find the rest.
                </p>
              )}
            </div>
          )}
        </>
      )}

      {confirmDelete && selectedItems.length > 0 && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm"
          onClick={cancelDelete}
          role="dialog"
          aria-modal="true"
          aria-labelledby="recycle-delete-title"
        >
          <div
            className="w-full max-w-lg rounded-xl bg-lunar-surface border border-lunar-border shadow-2xl flex flex-col overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="px-5 py-4 border-b border-lunar-border flex items-start justify-between gap-4">
              <div>
                <h2 id="recycle-delete-title" className="text-sm font-semibold text-lunar-white tracking-tight">
                  Delete {selectedItems.length === 1 ? `"${selectedItems[0].name}"` : `${selectedItems.length.toLocaleString()} items`} forever?
                </h2>
                <p className="text-xs text-lunar-text-sec mt-1">{formatStorageBytes(selectedBytes)} will be freed.</p>
              </div>
              <button
                type="button"
                onClick={cancelDelete}
                aria-label="Close"
                className="p-1.5 rounded-md text-lunar-muted hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="p-5 space-y-3">
              <div className="max-h-48 overflow-y-auto rounded-lg bg-lunar-bg border border-lunar-border divide-y divide-lunar-border">
                {selectedItems.slice(0, 50).map((i) => (
                  <div key={i.path} className="flex items-center justify-between gap-3 px-3 py-1.5">
                    <span className="text-[11px] text-lunar-text truncate" title={i.name}>
                      {i.name}
                    </span>
                    <span className="text-[10px] font-mono text-lunar-text-sec shrink-0">{formatStorageBytes(i.sizeBytes)}</span>
                  </div>
                ))}
                {selectedItems.length > 50 && (
                  <div className="px-3 py-1.5 text-[10px] text-lunar-muted">
                    and {(selectedItems.length - 50).toLocaleString()} more
                  </div>
                )}
              </div>
              <div className="flex items-start gap-2.5 px-3 py-2.5 rounded-lg bg-lunar-warning/10 border border-lunar-warning/30">
                <AlertTriangle className="w-3.5 h-3.5 text-lunar-warning shrink-0 mt-0.5" />
                <p className="text-[11px] text-lunar-text leading-relaxed">
                  These are deleted permanently and can no longer be restored. Everything else stays in the Recycle Bin.
                </p>
              </div>
            </div>
            <div className="px-5 py-3 border-t border-lunar-border bg-lunar-surface-2/40 flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={cancelDelete}
                className="px-3.5 py-2 rounded text-xs font-medium text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => void confirmDeleteSelected()}
                className="inline-flex items-center gap-2 px-3.5 py-2 rounded text-xs font-medium border bg-lunar-critical text-lunar-white border-lunar-critical hover:opacity-90 transition-opacity cursor-pointer"
              >
                <Trash2 className="w-3.5 h-3.5" />
                <span>Delete forever</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
