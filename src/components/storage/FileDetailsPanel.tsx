import React from "react";
import {
  X,
  Folder,
  FileText,
  ExternalLink,
  FolderOpen,
} from "lucide-react";
import {
  STORAGE_CATEGORY_META,
  STORAGE_IMPORTANCE_META,
  type StorageItem,
} from "../../types/storage";
import {
  formatStorageBytes,
  formatStorageTimestampDetailed,
} from "../../services/storage";

interface FileDetailsPanelProps {
  item: StorageItem;
  onClose: () => void;
  onOpenLocation: (path: string) => void;
  onBrowseFolder: (path: string) => void;
}

export const FileDetailsPanel: React.FC<FileDetailsPanelProps> = ({
  item,
  onClose,
  onOpenLocation,
  onBrowseFolder,
}) => {
  const catMeta =
    STORAGE_CATEGORY_META[item.category] ?? STORAGE_CATEGORY_META.UNKNOWN;
  const impMeta =
    STORAGE_IMPORTANCE_META[item.importance] ?? STORAGE_IMPORTANCE_META.UNKNOWN;

  return (
    <aside className="w-88 bg-lunar-surface border-l border-lunar-border flex flex-col h-full shrink-0 overflow-hidden">
      {/* Header */}
      <div className="px-4 py-3 border-b border-lunar-border flex items-center justify-between gap-2 bg-lunar-surface-2/50 shrink-0">
        <span className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-muted">
          File Details
        </span>
        <button
          type="button"
          onClick={onClose}
          className="p-1 rounded hover:bg-lunar-elevated text-lunar-muted hover:text-lunar-white transition-colors cursor-pointer"
          title="Close inspector"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Scrollable Inspector Body */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* Item Name & Icon */}
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 rounded bg-lunar-bg border border-lunar-border flex items-center justify-center shrink-0 mt-0.5">
            {item.isDir ? (
              <Folder className="w-4 h-4 text-lunar-white" />
            ) : (
              <FileText className="w-4 h-4 text-lunar-text-sec" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <h3 className="text-sm font-semibold text-lunar-white break-all leading-snug">
              {item.name}
            </h3>
            <span className="text-[11px] font-mono text-lunar-muted">
              {item.itemType}
            </span>
          </div>
        </div>

        {/* Metadata Fields */}
        <div className="space-y-3 pt-1">
          <div>
            <div className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted mb-1">
              Path
            </div>
            <div className="text-xs font-mono text-lunar-text bg-lunar-bg border border-lunar-border rounded px-2.5 py-1.5 break-all select-text">
              {item.path || "Unavailable"}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <div className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted mb-1">
                Size
              </div>
              <div className="text-xs font-mono font-semibold text-lunar-white">
                {formatStorageBytes(item.size)}
              </div>
            </div>

            <div>
              <div className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted mb-1">
                Type
              </div>
              <div className="text-xs font-mono text-lunar-text">
                {item.itemType || "Unavailable"}
              </div>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="text-center">
              <div className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted mb-1">
                Category
              </div>
              <span
                className={`inline-block text-[11px] font-mono text-center ${catMeta.badgeClass}`}
              >
                {catMeta.shortLabel}
              </span>
            </div>

            <div className="text-center">
              <div className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted mb-1">
                Importance
              </div>
              <span
                className={`inline-block text-[11px] font-mono text-center ${impMeta.badgeClass}`}
              >
                {impMeta.label}
              </span>
            </div>
          </div>

          <div className="space-y-2.5 pt-2 border-t border-lunar-border/60">
            <div>
              <div className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted">
                Created
              </div>
              <div className="text-xs font-mono text-lunar-text-sec mt-0.5">
                {formatStorageTimestampDetailed(item.createdMs)}
              </div>
            </div>

            <div>
              <div className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted">
                Modified
              </div>
              <div className="text-xs font-mono text-lunar-text-sec mt-0.5">
                {formatStorageTimestampDetailed(item.modifiedMs)}
              </div>
            </div>

            <div>
              <div className="text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted">
                Last Accessed
              </div>
              <div className="text-xs font-mono text-lunar-text-sec mt-0.5">
                {formatStorageTimestampDetailed(item.accessedMs)}
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Footer Actions */}
      <div className="p-4 border-t border-lunar-border bg-lunar-surface-2/40 space-y-2 shrink-0">
        {item.isDir && (
          <button
            type="button"
            onClick={() => onBrowseFolder(item.path)}
            className="w-full inline-flex items-center justify-center gap-2 px-3 py-2 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
          >
            <FolderOpen className="w-3.5 h-3.5" />
            <span>Open in Filesystem Explorer</span>
          </button>
        )}

        <button
          type="button"
          onClick={() => onOpenLocation(item.path)}
          className="w-full inline-flex items-center justify-center gap-2 px-3 py-2 rounded bg-lunar-surface hover:bg-lunar-elevated text-xs font-medium text-lunar-text hover:text-lunar-white border border-lunar-border transition-colors cursor-pointer"
        >
          <ExternalLink className="w-3.5 h-3.5" />
          <span>Open Location</span>
        </button>
      </div>
    </aside>
  );
};
