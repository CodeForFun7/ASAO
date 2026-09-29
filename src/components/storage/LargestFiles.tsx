import React, { useMemo } from "react";
import { FileText } from "lucide-react";
import {
  STORAGE_CATEGORY_META,
  type LargestFilesSortField,
  type StorageItem,
} from "../../types/storage";
import {
  formatStorageBytes,
  formatStorageTimestamp,
} from "../../services/storage";

interface LargestFilesProps {
  files: StorageItem[];
  sortField: LargestFilesSortField;
  onSortChange: (field: LargestFilesSortField) => void;
  selectedPath: string | null;
  onSelectFile: (file: StorageItem) => void;
}

export const LargestFiles: React.FC<LargestFilesProps> = ({
  files,
  sortField,
  onSortChange,
  selectedPath,
  onSelectFile,
}) => {
  const sortedFiles = useMemo(() => {
    const copy = [...files];
    copy.sort((a, b) => {
      if (sortField === "modified") {
        return (b.modifiedMs ?? 0) - (a.modifiedMs ?? 0) || b.size - a.size;
      }
      if (sortField === "accessed") {
        return (b.accessedMs ?? 0) - (a.accessedMs ?? 0) || b.size - a.size;
      }
      return b.size - a.size;
    });
    return copy.slice(0, 8);
  }, [files, sortField]);

  return (
    <div className="rounded-lg lunar-glass-card overflow-hidden flex flex-col">
      {/* Section Header + Sort Selector */}
      <div className="px-4 py-3 border-b border-lunar-border flex flex-wrap items-center justify-between gap-2 bg-lunar-surface-2/40">
        <div>
          <h2 className="text-xs font-semibold text-lunar-white tracking-tight">
            Largest Files
          </h2>
          <p className="text-[11px] text-lunar-muted mt-0.5">
            Select any file to inspect its filesystem metadata
          </p>
        </div>

        {/* Sort pills: Size | Last Modified | Last Accessed */}
        <div className="flex items-center gap-1 bg-lunar-bg border border-lunar-border rounded p-0.5 text-[10px] font-mono">
          <button
            type="button"
            onClick={() => onSortChange("size")}
            className={`px-2 py-0.5 rounded transition-colors cursor-pointer ${
              sortField === "size"
                ? "bg-lunar-elevated text-lunar-white"
                : "text-lunar-muted hover:text-lunar-text"
            }`}
          >
            Size
          </button>
          <button
            type="button"
            onClick={() => onSortChange("modified")}
            className={`px-2 py-0.5 rounded transition-colors cursor-pointer ${
              sortField === "modified"
                ? "bg-lunar-elevated text-lunar-white"
                : "text-lunar-muted hover:text-lunar-text"
            }`}
          >
            Modified
          </button>
          <button
            type="button"
            onClick={() => onSortChange("accessed")}
            className={`px-2 py-0.5 rounded transition-colors cursor-pointer ${
              sortField === "accessed"
                ? "bg-lunar-elevated text-lunar-white"
                : "text-lunar-muted hover:text-lunar-text"
            }`}
          >
            Accessed
          </button>
        </div>
      </div>

      {/* Column Header */}
      <div className="grid grid-cols-12 items-center gap-2 px-4 py-2 bg-lunar-surface-2 border-b border-lunar-border text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted">
        <div className="col-span-6">File</div>
        <div className="col-span-3 text-center">Category</div>
        <div className="col-span-3 text-right">Size</div>
      </div>

      {/* File Rows */}
      <div className="divide-y divide-lunar-border/40 max-h-72 overflow-y-auto">
        {sortedFiles.length === 0 ? (
          <div className="py-10 text-center text-xs text-lunar-muted font-mono">
            No large files indexed on this volume.
          </div>
        ) : (
          sortedFiles.map((file) => {
            const catMeta =
              STORAGE_CATEGORY_META[file.category] ??
              STORAGE_CATEGORY_META.UNKNOWN;
            const isSelected =
              selectedPath !== null &&
              selectedPath.toLowerCase() === file.path.toLowerCase();

            return (
              <div
                key={file.path}
                onClick={() => onSelectFile(file)}
                className={`grid grid-cols-12 items-center gap-2 px-4 py-2.5 text-xs transition-colors cursor-pointer ${
                  isSelected
                    ? "bg-lunar-elevated text-lunar-white"
                    : "hover:bg-lunar-surface-2/80 text-lunar-text"
                }`}
                title={file.path}
              >
                <div className="col-span-6 flex items-center gap-2.5 min-w-0">
                  <FileText className="w-3.5 h-3.5 text-lunar-muted shrink-0" />
                  <div className="min-w-0 flex-1">
                    <div className="font-medium text-lunar-white truncate">
                      {file.name}
                    </div>
                    <div className="text-[10px] font-mono text-lunar-muted truncate">
                      {sortField === "modified"
                        ? `Modified ${formatStorageTimestamp(file.modifiedMs)}`
                        : sortField === "accessed"
                        ? `Accessed ${formatStorageTimestamp(file.accessedMs)}`
                        : file.path}
                    </div>
                  </div>
                </div>

                <div className="col-span-3 flex items-center justify-center text-center">
                  <span
                    className={`text-[10px] font-mono text-center whitespace-nowrap ${catMeta.badgeClass}`}
                  >
                    {catMeta.shortLabel}
                  </span>
                </div>

                <div className="col-span-3 text-right font-mono font-medium text-lunar-white">
                  {formatStorageBytes(file.size)}
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
