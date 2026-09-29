import React from "react";
import {
  ChevronDown,
  ChevronRight,
  Flame,
  FolderSync,
  Layers,
  PowerOff,
  ShieldCheck,
  Terminal,
  Cpu,
  Database,
} from "lucide-react";
import type { StartupGroup, StartupItem } from "../../types/startup";
import { STARTUP_GROUP_DESCRIPTIONS } from "../../types/startup";
import { StartupItemRow } from "./StartupItemRow";
import { formatBytes } from "../../services/tauri";

interface StartupGroupAccordionProps {
  group: StartupGroup;
  items: StartupItem[];
  isExpanded: boolean;
  selectedItemId: string | null;
  actionInProgressId: string | null;
  onToggleExpand: () => void;
  onSelectItem: (id: string) => void;
  onToggleItemState: (item: StartupItem, e: React.MouseEvent) => void;
}

export const StartupGroupAccordion: React.FC<StartupGroupAccordionProps> = ({
  group,
  items,
  isExpanded,
  selectedItemId,
  actionInProgressId,
  onToggleExpand,
  onSelectItem,
  onToggleItemState,
}) => {
  // Group icons
  const GroupIcon = {
    "Registry Startup": Terminal,
    "Startup Folder": FolderSync,
    "Scheduled Tasks": Layers,
    "Windows Services": Database,
    "Winlogon / System Startup": ShieldCheck,
    "Other Autostart Mechanisms": Cpu,
  }[group];

  // Summary computations
  const totalCount = items.length;
  const highImpactCount = items.filter((i) => i.impact === "high").length;
  const disabledCount = items.filter((i) => !i.isEnabled).length;
  const runningCount = items.filter((i) => i.isCurrentlyRunning).length;
  const cumulativeCpu = items.reduce((acc, i) => acc + i.bootCpuMs, 0);
  const cumulativeDisk = items.reduce((acc, i) => acc + i.bootDiskBytes, 0);

  const description = STARTUP_GROUP_DESCRIPTIONS[group];

  return (
    <div className="rounded-xl border border-lunar-border bg-lunar-surface/70 overflow-hidden shadow-xs transition-colors">
      {/* Collapsible Accordion Header using the Exact Same Column Structure */}
      <button
        type="button"
        onClick={onToggleExpand}
        className="w-full px-[22px] py-3 flex items-center text-left hover:bg-lunar-surface-2/80 transition-colors cursor-pointer select-none"
      >
        {/* 1. Category Identity Column (Aligned with item Identity) */}
        <div className="flex-1 min-w-0 pr-4 flex items-center gap-2.5">
          <div className="w-4 h-4 flex items-center justify-center text-lunar-text-sec shrink-0">
            {isExpanded ? (
              <ChevronDown className="w-4 h-4 text-lunar-white" />
            ) : (
              <ChevronRight className="w-4 h-4 text-lunar-muted" />
            )}
          </div>

          <div className="w-6 h-6 rounded-md bg-lunar-elevated border border-lunar-border flex items-center justify-center shrink-0">
            <GroupIcon className="w-3.5 h-3.5 text-lunar-text-sec" />
          </div>

          <span className="text-xs font-semibold text-lunar-white tracking-tight shrink-0">
            {group}
          </span>

          <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-text-sec border border-lunar-border shrink-0">
            {totalCount}
          </span>

          <span className="text-[11px] text-lunar-text-sec/70 truncate min-w-0 hidden xl:inline">
            {description}
          </span>
        </div>

        {/* 2. Status / Impact Column: Fixed-width (w-24), Aligned with item Status */}
        <div className="w-24 shrink-0 flex items-center gap-1.5">
          {disabledCount > 0 && (
            <span
              title={`${disabledCount} autostart entries currently disabled`}
              className="inline-flex items-center gap-1 font-mono text-[10px] px-1.5 py-0.5 rounded bg-lunar-critical/10 text-lunar-critical border border-lunar-critical/20 cursor-help shrink-0"
            >
              <PowerOff className="w-2.5 h-2.5" />
              <span>{disabledCount}</span>
            </span>
          )}

          {highImpactCount > 0 && (
            <span
              title={`${highImpactCount} high impact autostart items`}
              className="inline-flex items-center gap-1 font-mono text-[10px] text-lunar-critical bg-lunar-critical/10 px-1.5 py-0.5 rounded border border-lunar-critical/25 cursor-help shrink-0"
            >
              <Flame className="w-2.5 h-2.5" />
              <span>{highImpactCount}</span>
            </span>
          )}
        </div>

        {/* 3. Summary Metrics Column: Fixed-width (w-64), Aligned with item Metrics */}
        <div className="w-64 shrink-0 font-mono text-[10px] text-lunar-muted grid grid-cols-4 gap-1.5 items-center">
          <span
            title="Total currently active processes in this group"
            className="truncate text-lunar-text-sec cursor-help"
          >
            {runningCount} active
          </span>
          <span
            title="Cumulative CPU time for items in this group"
            className="truncate text-lunar-text-sec cursor-help"
          >
            {cumulativeCpu}ms CPU
          </span>
          <span
            title="Cumulative startup disk I/O in this group"
            className="truncate text-lunar-text-sec cursor-help"
          >
            {formatBytes(cumulativeDisk)}
          </span>
          <span className="text-lunar-border/40 text-center">—</span>
        </div>

        {/* 4. Impact Column Spacer: Fixed-width (w-12) */}
        <div className="w-12 shrink-0 flex items-center justify-center">
          <span className="w-1.5 h-1.5 rounded-full bg-lunar-border/60" />
        </div>

        {/* 5. Actions Column Spacer: Fixed-width (w-24) */}
        <div className="w-24 shrink-0" />
      </button>

      {/* Accordion Content Body */}
      {isExpanded && (
        <div className="p-2 pt-1 border-t border-lunar-border/50 bg-lunar-bg/40 space-y-1.5">
          {items.length === 0 ? (
            <div className="py-5 text-center text-xs text-lunar-muted font-mono">
              No startup entries detected in this category.
            </div>
          ) : (
            items.map((item) => (
              <StartupItemRow
                key={item.id}
                item={item}
                isSelected={selectedItemId === item.id}
                isActionLoading={actionInProgressId === item.id}
                onSelect={() => onSelectItem(item.id)}
                onToggleState={(e) => onToggleItemState(item, e)}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
};
