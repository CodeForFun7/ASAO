import React from "react";
import {
  ChevronDown,
  ChevronRight,
  Cloud,
  EyeOff,
  FolderOpen,
  Package,
  PackageX,
  ShieldAlert,
} from "lucide-react";
import type { ResidualCandidate, ResidualGroup } from "../../../types/residual";
import { formatStorageBytes, formatStorageTimestamp } from "../../../services/storage";
import {
  CategoryLabel,
  ConfidenceBadge,
  LeftoverCheckbox,
  checkStateFor,
} from "./LeftoverPrimitives";

interface LeftoverGroupCardProps {
  group: ResidualGroup;
  /** Folders of this group shown in this section (confident or uncertain). */
  folders: ResidualCandidate[];
  expanded: boolean;
  busy: boolean;
  isSelected: (path: string) => boolean;
  onToggleExpand: () => void;
  onToggleFolder: (path: string) => void;
  onSetGroup: (folders: ResidualCandidate[], value: boolean) => void;
  onOpenFolder: (path: string) => void;
  onIgnoreFolder: (path: string) => void;
  onIgnoreApp: (group: ResidualGroup) => void;
}

export function totalSize(folders: ResidualCandidate[]): number | null {
  if (folders.some((f) => f.sizeBytes === null)) {
    const known = folders.reduce((acc, f) => acc + (f.sizeBytes ?? 0), 0);
    return known > 0 ? known : null;
  }
  return folders.reduce((acc, f) => acc + (f.sizeBytes ?? 0), 0);
}

function newest(folders: ResidualCandidate[]): number | null {
  const times = folders.map((f) => f.newestModifiedMs).filter((t): t is number => t !== null);
  return times.length ? Math.max(...times) : null;
}

function confidenceOf(folders: ResidualCandidate[]) {
  if (folders.some((f) => f.confidence === "high")) return "high" as const;
  if (folders.some((f) => f.confidence === "medium")) return "medium" as const;
  return "low" as const;
}

export const LeftoverGroupCard: React.FC<LeftoverGroupCardProps> = ({
  group,
  folders,
  expanded,
  busy,
  isSelected,
  onToggleExpand,
  onToggleFolder,
  onSetGroup,
  onOpenFolder,
  onIgnoreFolder,
  onIgnoreApp,
}) => {
  const selectedCount = folders.filter((f) => isSelected(f.path)).length;
  const state = checkStateFor(selectedCount, folders.length);
  const size = totalSize(folders);
  const sizing = folders.some((f) => f.sizeBytes === null);
  const lastUsed = newest(folders);
  const GroupIcon = group.isUnknown ? PackageX : Package;

  return (
    <div className="rounded-xl border border-lunar-border bg-lunar-surface/70 overflow-hidden shadow-xs transition-colors">
      <div
        role="button"
        tabIndex={0}
        onClick={onToggleExpand}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onToggleExpand();
          }
        }}
        className="w-full px-4 py-3 flex items-center gap-3 text-left hover:bg-lunar-surface-2/80 transition-colors cursor-pointer select-none"
      >
        <LeftoverCheckbox
          state={state}
          disabled={busy}
          label={`Select all folders of ${group.appName}`}
          onToggle={() => onSetGroup(folders, state !== "checked")}
        />

        <div className="w-4 h-4 flex items-center justify-center shrink-0">
          {expanded ? (
            <ChevronDown className="w-4 h-4 text-lunar-white" />
          ) : (
            <ChevronRight className="w-4 h-4 text-lunar-muted" />
          )}
        </div>

        <div className="w-6 h-6 rounded-md bg-lunar-elevated border border-lunar-border flex items-center justify-center shrink-0">
          <GroupIcon className="w-3.5 h-3.5 text-lunar-text-sec" />
        </div>

        <div className="flex-1 min-w-0 flex items-center gap-2">
          <span className="text-xs font-semibold text-lunar-white tracking-tight truncate" title={group.appName}>
            {group.appName}
          </span>
          <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-text-sec border border-lunar-border shrink-0">
            {folders.length} {folders.length === 1 ? "folder" : "folders"}
          </span>
          <ConfidenceBadge confidence={confidenceOf(folders)} />
          {group.removedOnMs ? (
            <span className="text-[11px] text-lunar-text-sec/80 truncate hidden lg:inline">
              Uninstalled around {formatStorageTimestamp(group.removedOnMs)}
            </span>
          ) : group.lastRunMs ? (
            <span className="text-[11px] text-lunar-text-sec/80 truncate hidden lg:inline">
              Last run {formatStorageTimestamp(group.lastRunMs)}, program now gone
            </span>
          ) : null}
        </div>

        <div className="w-28 shrink-0 text-right">
          <div className="text-[10px] text-lunar-muted">Last used</div>
          <div className="text-[11px] font-mono text-lunar-text-sec">
            {lastUsed ? formatStorageTimestamp(lastUsed) : "No files"}
          </div>
        </div>

        <div className="w-24 shrink-0 text-right">
          <div className="text-[10px] text-lunar-muted">Size</div>
          <div className={`text-xs font-mono font-semibold ${sizing ? "text-lunar-muted animate-pulse" : "text-lunar-white"}`}>
            {size === null ? "Measuring" : formatStorageBytes(size)}
          </div>
        </div>
      </div>

      {expanded && (
        <div className="p-2 pt-1 border-t border-lunar-border/50 bg-lunar-bg/40 space-y-1.5">
          {folders.map((f) => (
            <div
              key={f.id}
              className={`px-3.5 py-2.5 rounded-lg border flex items-start gap-3 transition-colors ${
                isSelected(f.path)
                  ? "bg-lunar-elevated border-lunar-text-sec/40"
                  : "bg-lunar-surface border-lunar-border/80 hover:bg-lunar-surface-2"
              }`}
            >
              <div className="pt-0.5">
                <LeftoverCheckbox
                  state={isSelected(f.path) ? "checked" : "unchecked"}
                  disabled={busy}
                  label={`Select ${f.path}`}
                  onToggle={() => onToggleFolder(f.path)}
                />
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="text-xs font-semibold text-lunar-white truncate" title={f.name}>
                    {f.name}
                  </span>
                  <CategoryLabel category={f.category} />
                  <ConfidenceBadge confidence={f.confidence} />
                  {f.needsAdmin && (
                    <span title="Needs administrator permission. Windows will ask when you remove it." className="cursor-help shrink-0">
                      <ShieldAlert className="w-3.5 h-3.5 text-lunar-warning" />
                    </span>
                  )}
                  {f.inOnedrive && (
                    <span title="Inside OneDrive. Removing it also removes it from your other synced devices." className="cursor-help shrink-0">
                      <Cloud className="w-3.5 h-3.5 text-lunar-warning" />
                    </span>
                  )}
                </div>
                <div className="font-mono text-[10px] text-lunar-muted truncate mt-0.5" title={f.path}>
                  {f.path}
                </div>
                {f.reasons.length > 0 && (
                  <div className="text-[10px] text-lunar-text-sec/80 mt-1 leading-relaxed">
                    {f.reasons.join(" · ")}
                  </div>
                )}
              </div>

              <div className="w-24 shrink-0 text-right">
                <div className="text-[11px] font-mono text-lunar-text-sec">
                  {f.newestModifiedMs ? formatStorageTimestamp(f.newestModifiedMs) : "No files"}
                </div>
                <div className={`text-xs font-mono font-semibold mt-0.5 ${f.sizeBytes === null ? "text-lunar-muted animate-pulse" : "text-lunar-white"}`}>
                  {f.sizeBytes === null ? "Measuring" : formatStorageBytes(f.sizeBytes)}
                </div>
              </div>

              <div className="flex items-center gap-1.5 shrink-0">
                <button
                  type="button"
                  onClick={() => onOpenFolder(f.path)}
                  title="Open in Explorer"
                  className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg bg-lunar-elevated hover:bg-lunar-border text-[11px] font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
                >
                  <FolderOpen className="w-3 h-3" />
                  <span>Open in Explorer</span>
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onIgnoreFolder(f.path)}
                  title="Don't show this folder again"
                  aria-label="Don't show this folder again"
                  className="inline-flex items-center justify-center w-7 h-7 rounded-lg bg-lunar-surface hover:bg-lunar-elevated text-lunar-text-sec hover:text-lunar-white border border-lunar-border transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
                >
                  <EyeOff className="w-3 h-3" />
                </button>
              </div>
            </div>
          ))}

          {!group.isUnknown && (
            <div className="flex justify-end px-1 pt-1">
              <button
                type="button"
                disabled={busy}
                onClick={() => onIgnoreApp(group)}
                className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded text-[11px] text-lunar-text-sec hover:text-lunar-white transition-colors cursor-pointer disabled:opacity-45 disabled:cursor-not-allowed"
              >
                <EyeOff className="w-3 h-3" />
                <span>Don't show {group.appName} again</span>
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
