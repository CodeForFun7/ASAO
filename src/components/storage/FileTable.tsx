import React from "react";
import {
  Folder,
  FileText,
  FileArchive,
  FileCode,
  Film,
  Image as ImageIcon,
  Cpu,
  Lock,
  RefreshCw,
  FolderOpen,
  SearchX,
  ChevronRight,
} from "lucide-react";
import {
  STORAGE_CATEGORY_META,
  STORAGE_IMPORTANCE_META,
  type StorageItem,
  type StorageSortDirection,
  type StorageSortField,
} from "../../types/storage";
import {
  formatStorageBytes,
  formatStorageTimestamp,
} from "../../services/storage";

interface FileTableProps {
  items: StorageItem[];
  totalUnfilteredCount: number;
  isLoading: boolean;
  permissionDenied: boolean;
  selectedPath: string | null;
  sortField: StorageSortField;
  sortDirection: StorageSortDirection;
  onSortColumn: (field: StorageSortField) => void;
  onSelectItem: (item: StorageItem) => void;
  onOpenFolder: (path: string) => void;
  onRetryPermission: () => void;
  onResetFilters: () => void;
}

function getFileIcon(item: StorageItem) {
  if (item.isDir) return Folder;
  const typeLower = item.itemType.toLowerCase();
  if (typeLower.includes("archive")) return FileArchive;
  if (typeLower.includes("video")) return Film;
  if (typeLower.includes("image")) return ImageIcon;
  if (typeLower.includes("source") || typeLower.includes("config")) return FileCode;
  if (typeLower.includes("application") || typeLower.includes("binary")) return Cpu;
  return FileText;
}

export const FileTable: React.FC<FileTableProps> = ({
  items,
  totalUnfilteredCount,
  isLoading,
  permissionDenied,
  selectedPath,
  sortField,
  sortDirection,
  onSortColumn,
  onSelectItem,
  onOpenFolder,
  onRetryPermission,
  onResetFilters,
}) => {
  const renderSortIndicator = (field: StorageSortField) => {
    if (sortField !== field) return null;
    return (
      <span className="ml-1 text-lunar-white">
        {sortDirection === "desc" ? "↓" : "↑"}
      </span>
    );
  };

  if (permissionDenied) {
    return (
      <div className="py-16 px-6 flex flex-col items-center justify-center text-center">
        <Lock className="w-7 h-7 text-lunar-warning mb-3 stroke-[1.5]" />
        <p className="text-sm font-medium text-lunar-white">
          Unable to access this location.
        </p>
        <p className="text-xs text-lunar-text-sec mt-1 max-w-sm">
          ASAO doesn't currently have permission to read this folder.
        </p>
        <button
          type="button"
          onClick={onRetryPermission}
          className="mt-4 inline-flex items-center gap-2 px-3.5 py-1.5 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
        >
          <RefreshCw className="w-3.5 h-3.5" />
          <span>Retry</span>
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col">
      {/* Table Header */}
      <div className="grid grid-cols-12 items-center gap-3 px-4 py-2.5 bg-lunar-surface-2 border-b border-lunar-border text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted select-none">
        <button
          type="button"
          onClick={() => onSortColumn("name")}
          className="col-span-4 text-left hover:text-lunar-white transition-colors cursor-pointer flex items-center"
        >
          <span>Name</span>
          {renderSortIndicator("name")}
        </button>
        <div className="col-span-1">Type</div>
        <button
          type="button"
          onClick={() => onSortColumn("size")}
          className="col-span-2 text-right hover:text-lunar-white transition-colors cursor-pointer flex items-center justify-end"
        >
          <span>Size</span>
          {renderSortIndicator("size")}
        </button>
        <button
          type="button"
          onClick={() => onSortColumn("category")}
          className="col-span-2 text-center hover:text-lunar-white transition-colors cursor-pointer flex items-center justify-center"
        >
          <span>Category</span>
          {renderSortIndicator("category")}
        </button>
        <button
          type="button"
          onClick={() => onSortColumn("importance")}
          className="col-span-2 text-center hover:text-lunar-white transition-colors cursor-pointer flex items-center justify-center"
        >
          <span>Importance</span>
          {renderSortIndicator("importance")}
        </button>
        <button
          type="button"
          onClick={() => onSortColumn("modified")}
          className="col-span-1 text-right hover:text-lunar-white transition-colors cursor-pointer flex items-center justify-end"
        >
          <span>Modified</span>
          {renderSortIndicator("modified")}
        </button>
      </div>

      {/* Rows or Empty States */}
      {isLoading ? (
        <div className="py-16 flex flex-col items-center justify-center text-center">
          <div className="w-6 h-6 rounded-full border-2 border-lunar-border border-t-lunar-white animate-spin mb-3" />
          <p className="text-xs font-mono text-lunar-muted">
            Reading filesystem directory...
          </p>
        </div>
      ) : items.length === 0 ? (
        totalUnfilteredCount === 0 ? (
          <div className="py-16 px-6 flex flex-col items-center justify-center text-center">
            <FolderOpen className="w-7 h-7 text-lunar-muted mb-2.5 stroke-[1.5]" />
            <p className="text-sm font-medium text-lunar-text">
              This folder is empty.
            </p>
          </div>
        ) : (
          <div className="py-16 px-6 flex flex-col items-center justify-center text-center">
            <SearchX className="w-7 h-7 text-lunar-muted mb-2.5 stroke-[1.5]" />
            <p className="text-sm font-medium text-lunar-text">
              No files found.
            </p>
            <button
              type="button"
              onClick={onResetFilters}
              className="mt-3 px-3 py-1.5 rounded bg-lunar-elevated hover:bg-lunar-border text-xs text-lunar-white border border-lunar-border transition-colors cursor-pointer"
            >
              Reset Filters
            </button>
          </div>
        )
      ) : (
        <div className="divide-y divide-lunar-border/40 max-h-[460px] overflow-y-auto">
          {items.map((item) => {
            const Icon = getFileIcon(item);
            const catMeta =
              STORAGE_CATEGORY_META[item.category] ??
              STORAGE_CATEGORY_META.UNKNOWN;
            const impMeta =
              STORAGE_IMPORTANCE_META[item.importance] ??
              STORAGE_IMPORTANCE_META.UNKNOWN;
            const isSelected =
              selectedPath !== null &&
              selectedPath.toLowerCase() === item.path.toLowerCase();

            return (
              <div
                key={item.path}
                onClick={() => onSelectItem(item)}
                onDoubleClick={() => {
                  if (item.isDir) {
                    onOpenFolder(item.path);
                  }
                }}
                className={`grid grid-cols-12 items-center gap-3 px-4 py-2.5 text-xs transition-colors cursor-pointer group ${
                  isSelected
                    ? "bg-lunar-elevated text-lunar-white"
                    : "hover:bg-lunar-surface-2/80 text-lunar-text"
                }`}
              >
                {/* Name */}
                <div className="col-span-4 flex items-center gap-2.5 min-w-0">
                  <Icon
                    className={`w-4 h-4 shrink-0 ${
                      item.isDir
                        ? "text-lunar-white"
                        : "text-lunar-muted group-hover:text-lunar-text-sec"
                    }`}
                  />
                  <div className="min-w-0 flex-1 flex items-center gap-1.5">
                    <span
                      className={`truncate ${
                        item.isDir
                          ? "font-medium text-lunar-white"
                          : "text-lunar-text"
                      }`}
                    >
                      {item.name}
                    </span>
                    {item.isDir && (
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          onOpenFolder(item.path);
                        }}
                        className="px-1.5 py-0.5 rounded bg-lunar-bg hover:bg-lunar-border text-[10px] font-mono text-lunar-text-sec hover:text-lunar-white border border-lunar-border opacity-0 group-hover:opacity-100 transition-opacity inline-flex items-center gap-0.5 shrink-0 cursor-pointer"
                        title="Open folder"
                      >
                        <span>Open</span>
                        <ChevronRight className="w-2.5 h-2.5" />
                      </button>
                    )}
                  </div>
                </div>

                {/* Type */}
                <div className="col-span-1 text-lunar-text-sec font-mono text-[11px] truncate">
                  {item.itemType}
                </div>

                {/* Size */}
                <div className="col-span-2 text-right font-mono font-medium text-lunar-white">
                  {formatStorageBytes(item.size)}
                </div>

                {/* Category */}
                <div className="col-span-2 flex items-center justify-center text-center">
                  <span
                    className={`text-[10px] font-mono text-center whitespace-nowrap ${catMeta.badgeClass}`}
                  >
                    {catMeta.shortLabel}
                  </span>
                </div>

                {/* Importance */}
                <div className="col-span-2 flex items-center justify-center text-center">
                  <span
                    className={`text-[10px] font-mono text-center whitespace-nowrap ${impMeta.badgeClass}`}
                  >
                    {impMeta.label}
                  </span>
                </div>

                {/* Last Modified */}
                <div className="col-span-1 text-right font-mono text-[11px] text-lunar-muted truncate">
                  {formatStorageTimestamp(item.modifiedMs)}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
