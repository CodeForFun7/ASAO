import React from "react";
import { ChevronRight, CornerLeftUp, Folder } from "lucide-react";
import { parsePathBreadcrumbs } from "../../services/storage";

interface BreadcrumbsProps {
  currentPath: string;
  parentPath: string | null;
  onNavigate: (path: string) => void;
}

export const Breadcrumbs: React.FC<BreadcrumbsProps> = ({
  currentPath,
  parentPath,
  onNavigate,
}) => {
  const segments = parsePathBreadcrumbs(currentPath);

  return (
    <div className="flex items-center gap-1.5 flex-wrap text-xs font-mono">
      <button
        type="button"
        disabled={!parentPath}
        onClick={() => parentPath && onNavigate(parentPath)}
        className={`p-1 transition-colors ${
          parentPath
            ? "text-lunar-text-sec hover:text-lunar-white cursor-pointer"
            : "text-lunar-muted/40 cursor-not-allowed"
        }`}
        title={parentPath ? `Up to ${parentPath}` : "At volume root"}
      >
        <CornerLeftUp className="w-4 h-4" />
      </button>

      <div className="flex items-center gap-1 flex-wrap bg-lunar-surface border border-lunar-border rounded px-2.5 py-1 min-h-[28px]">
        <Folder className="w-3.5 h-3.5 text-lunar-muted mr-1 shrink-0" />
        {segments.length === 0 ? (
          <span className="text-lunar-muted">Root</span>
        ) : (
          segments.map((seg, idx) => {
            const isLast = idx === segments.length - 1;
            return (
              <React.Fragment key={seg.path}>
                {idx > 0 && (
                  <ChevronRight className="w-3 h-3 text-lunar-muted shrink-0" />
                )}
                <button
                  type="button"
                  onClick={() => onNavigate(seg.path)}
                  className={`px-1 py-0.5 rounded transition-colors cursor-pointer ${
                    isLast
                      ? "text-lunar-white font-semibold bg-lunar-elevated/70"
                      : "text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-surface-2"
                  }`}
                >
                  {seg.label}
                </button>
              </React.Fragment>
            );
          })
        )}
      </div>
    </div>
  );
};
