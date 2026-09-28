import React from "react";
import { Folder, ChevronRight } from "lucide-react";
import {
  STORAGE_CATEGORY_META,
  type StorageItem,
} from "../../types/storage";
import { formatStorageBytes } from "../../services/storage";

interface LargestFoldersProps {
  folders: StorageItem[];
  currentExplorerPath: string;
  onOpenFolder: (path: string) => void;
  onSelectFolder: (item: StorageItem) => void;
}

export const LargestFolders: React.FC<LargestFoldersProps> = ({
  folders,
  currentExplorerPath,
  onOpenFolder,
  onSelectFolder,
}) => {
  const maxFolderBytes = folders[0]?.size ?? 1;

  return (
    <div className="rounded-lg lunar-glass-card overflow-hidden flex flex-col h-full">
      {/* Section Header */}
      <div className="px-4 py-3 border-b border-lunar-border flex items-center justify-between gap-2 bg-lunar-surface-2/40">
        <div>
          <h2 className="text-xs font-semibold text-lunar-white tracking-tight">
            Largest Folders
          </h2>
          <p className="text-[11px] text-lunar-muted mt-0.5">
            Click any directory to open it in the Filesystem Explorer
          </p>
        </div>
        <span className="text-[10px] font-mono text-lunar-muted uppercase tracking-wider">
          Top {folders.length}
        </span>
      </div>

      {/* Table Header */}
      <div className="grid grid-cols-12 items-center gap-2 px-4 py-2 bg-lunar-surface-2 border-b border-lunar-border text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted">
        <div className="col-span-6">Folder</div>
        <div className="col-span-3 text-center">Category</div>
        <div className="col-span-3 text-right">Size</div>
      </div>

      {/* Folder Rows */}
      <div className="divide-y divide-lunar-border/40 max-h-72 overflow-y-auto">
        {folders.length === 0 ? (
          <div className="py-10 text-center text-xs text-lunar-muted font-mono">
            No folders discovered on this volume.
          </div>
        ) : (
          folders.slice(0, 8).map((folder) => {
            const catMeta =
              STORAGE_CATEGORY_META[folder.category] ??
              STORAGE_CATEGORY_META.UNKNOWN;
            const ratio =
              maxFolderBytes > 0
                ? Math.max(3, Math.min(100, (folder.size / maxFolderBytes) * 100))
                : 0;
            const isCurrent =
              currentExplorerPath.toLowerCase() === folder.path.toLowerCase();

            return (
              <div
                key={folder.path}
                onClick={() => {
                  onSelectFolder(folder);
                  onOpenFolder(folder.path);
                }}
                className={`grid grid-cols-12 items-center gap-2 px-4 py-2.5 text-xs transition-colors cursor-pointer group ${
                  isCurrent
                    ? "bg-lunar-elevated/90 text-lunar-white"
                    : "hover:bg-lunar-surface-2/80 text-lunar-text"
                }`}
                title={folder.path}
              >
                {/* Folder Name & Path Subtitle */}
                <div className="col-span-6 flex items-center gap-2.5 min-w-0">
                  <Folder className="w-3.5 h-3.5 text-lunar-text-sec group-hover:text-lunar-white shrink-0" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <span className="font-medium text-lunar-white truncate">
                        {folder.name}
                      </span>
                      <ChevronRight className="w-3 h-3 text-lunar-muted opacity-0 group-hover:opacity-100 transition-opacity shrink-0" />
                    </div>
                    <div className="text-[10px] font-mono text-lunar-muted truncate">
                      {folder.path}
                    </div>
                  </div>
                </div>

                {/* Category Text */}
                <div className="col-span-3 flex items-center justify-center text-center">
                  <span
                    className={`text-[10px] font-mono text-center whitespace-nowrap ${catMeta.badgeClass}`}
                  >
                    {catMeta.shortLabel}
                  </span>
                </div>

                {/* Size & subtle bar */}
                <div className="col-span-3 flex flex-col items-end justify-center gap-1">
                  <span className="font-mono font-medium text-lunar-white">
                    {formatStorageBytes(folder.size)}
                  </span>
                  <div className="w-20 h-1 bg-lunar-bg rounded-full overflow-hidden">
                    <div
                      className="h-full bg-lunar-white/60 rounded-full"
                      style={{ width: `${ratio}%` }}
                    />
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
