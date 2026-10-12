import React from "react";
import {
  AlertTriangle,
  Clock,
  Cpu,
  Database,
  Flame,
  HardDrive,
  HelpCircle,
  Power,
  RotateCcw,
  ShieldAlert,
  ShieldCheck,
  Zap,
} from "lucide-react";
import type { StartupItem } from "../../types/startup";
import { SOURCE_LABELS } from "../../types/startup";
import { formatBytes } from "../../services/tauri";

interface StartupItemRowProps {
  item: StartupItem;
  isSelected: boolean;
  isActionLoading: boolean;
  onSelect: () => void;
  onToggleState: (e: React.MouseEvent) => void;
}

export const StartupItemRow: React.FC<StartupItemRowProps> = ({
  item,
  isSelected,
  isActionLoading,
  onSelect,
  onToggleState,
}) => {
  const sourceLabel = SOURCE_LABELS[item.source] ?? item.source;

  // Impact Icon & Tooltip Config
  const impactConfig = {
    high: {
      icon: Flame,
      color: "text-lunar-critical",
      bg: "bg-lunar-critical/10 border-lunar-critical/30",
      tooltip: `High Startup Impact (~${item.bootDurationMs}ms delay, ${item.bootCpuMs}ms CPU, ${formatBytes(item.bootDiskBytes)} disk I/O)`,
    },
    medium: {
      icon: AlertTriangle,
      color: "text-lunar-warning",
      bg: "bg-lunar-warning/10 border-lunar-warning/30",
      tooltip: `Medium Startup Impact (~${item.bootDurationMs}ms delay, ${item.bootCpuMs}ms CPU)`,
    },
    low: {
      icon: Zap,
      color: "text-lunar-healthy",
      bg: "bg-lunar-healthy/10 border-lunar-healthy/30",
      tooltip: `Low Startup Impact (<${item.bootDurationMs}ms delay, minimal contention)`,
    },
  }[item.impact];

  // Recommendation Icon & Tooltip Config
  const recConfig = {
    disable: {
      icon: ShieldAlert,
      color: "text-lunar-critical",
      bg: "bg-lunar-critical/10 border-lunar-critical/25",
      tooltip: "Recommendation: Disable (Safe to disable; reduce boot time)",
    },
    investigate: {
      icon: HelpCircle,
      color: "text-lunar-warning",
      bg: "bg-lunar-warning/10 border-lunar-warning/25",
      tooltip: "Recommendation: Investigate (User-dependent utility; review if autostart is required)",
    },
    keep: {
      icon: ShieldCheck,
      color: "text-lunar-healthy",
      bg: "bg-lunar-healthy/10 border-lunar-healthy/25",
      tooltip: "Recommendation: Keep (Essential Windows component or core system driver)",
    },
  }[item.recommendation];

  const ImpactIcon = impactConfig.icon;
  const RecIcon = recConfig.icon;

  return (
    <div
      onClick={onSelect}
      className={`group relative px-3.5 py-2.5 rounded-lg border transition-all cursor-pointer flex items-center ${
        isSelected
          ? "bg-lunar-elevated border-lunar-text-sec/40 shadow-lg shadow-black/40 ring-1 ring-white/10"
          : "bg-lunar-surface border-lunar-border/80 hover:bg-lunar-surface-2 hover:border-lunar-border"
      } ${!item.isEnabled ? "opacity-60 hover:opacity-90" : ""}`}
    >
      {/* 1. Identity Column: App Name & Publisher/Source (Fixed Left, Flexible Width, Truncates with Ellipsis) */}
      <div className="flex-1 min-w-0 pr-4">
        <span
          title={item.name}
          className="font-semibold text-xs text-lunar-white truncate block"
        >
          {item.name}
        </span>
        <div className="text-[11px] text-lunar-text-sec truncate block mt-0.5">
          {item.publisher && (
            <span className="text-lunar-text-sec mr-1.5">{item.publisher}</span>
          )}
          <span className="font-mono text-[10px] text-lunar-muted">
            {sourceLabel}
          </span>
        </div>
      </div>

      {/* 2. Status / Icons Column: Fixed-width (w-24), Fixed-position */}
      <div className="w-24 shrink-0 flex items-center gap-1.5">
        {/* Running Status Icon: Green dot (PID on hover) or Inactive dot */}
        {item.isCurrentlyRunning ? (
          <span
            title={
              item.pid
                ? `PID: ${item.pid} (Running, ${formatBytes(item.memoryBytes)} RAM)`
                : `Running (${formatBytes(item.memoryBytes)} RAM)`
            }
            className="w-5 h-5 rounded flex items-center justify-center bg-lunar-healthy/15 border border-lunar-healthy/30 cursor-help shrink-0"
          >
            <span className="w-2 h-2 rounded-full bg-lunar-healthy animate-pulse" />
          </span>
        ) : (
          <span
            title="Inactive: Process is not currently running"
            className="w-5 h-5 rounded flex items-center justify-center bg-lunar-bg border border-lunar-border cursor-help shrink-0"
          >
            <span className="w-1.5 h-1.5 rounded-full bg-lunar-muted/60" />
          </span>
        )}

        {/* Recommendation Icon Badge */}
        <span
          title={recConfig.tooltip}
          className={`w-5 h-5 rounded flex items-center justify-center border cursor-help shrink-0 ${recConfig.bg} ${recConfig.color}`}
        >
          <RecIcon className="w-2.5 h-2.5" />
        </span>
      </div>

      {/* 3. Metrics Column: Fixed-width (w-64), Fixed-position Tabular Grid */}
      <div className="w-64 shrink-0 font-mono text-[10px] text-lunar-text-sec grid grid-cols-4 gap-1.5 items-center">
        <span
          title="CPU time consumed during initialization"
          className="flex items-center gap-1 cursor-help truncate"
        >
          <Cpu className="w-2.5 h-2.5 text-lunar-muted shrink-0" />
          <span className="truncate">{item.bootCpuMs}ms</span>
        </span>

        <span
          title="Disk I/O read/written during boot"
          className="flex items-center gap-1 cursor-help truncate"
        >
          <HardDrive className="w-2.5 h-2.5 text-lunar-muted shrink-0" />
          <span className="truncate">{formatBytes(item.bootDiskBytes)}</span>
        </span>

        <span
          title="Memory working set footprint"
          className="flex items-center gap-1 cursor-help truncate"
        >
          <Database className="w-2.5 h-2.5 text-lunar-muted shrink-0" />
          <span className="truncate">{formatBytes(item.memoryBytes)}</span>
        </span>

        <span
          title="Initialization duration until desktop ready"
          className="flex items-center gap-1 cursor-help truncate"
        >
          <Clock className="w-2.5 h-2.5 text-lunar-muted shrink-0" />
          <span className="truncate">~{item.bootDurationMs}ms</span>
        </span>
      </div>

      {/* 4. Impact Column: Fixed-width (w-12), Fixed-position Centered */}
      <div className="w-12 shrink-0 flex items-center justify-center">
        <span
          title={impactConfig.tooltip}
          className={`w-5 h-5 rounded flex items-center justify-center border cursor-help ${impactConfig.bg} ${impactConfig.color}`}
        >
          <ImpactIcon className="w-2.5 h-2.5" />
        </span>
      </div>

      {/* 5. Actions Column: Fixed-width (w-24), Fixed-position Right-aligned */}
      <div className="w-24 shrink-0 flex items-center justify-end">
        {item.classification === "essential" ? (
          <span
            className="w-20 py-1 text-[10px] font-mono text-lunar-muted rounded bg-lunar-bg border border-lunar-border text-center cursor-help block truncate"
            title="Protected Windows system component"
          >
            Protected
          </span>
        ) : (
          <button
            type="button"
            disabled={isActionLoading}
            onClick={onToggleState}
            title={item.isEnabled ? "Disable autostart" : "Re-enable autostart"}
            className={`w-20 py-1 rounded text-[11px] font-medium transition-colors border cursor-pointer inline-flex items-center justify-center gap-1 ${
              item.isEnabled
                ? "bg-lunar-surface-2 hover:bg-lunar-critical/20 text-lunar-text hover:text-lunar-critical border-lunar-border hover:border-lunar-critical/40"
                : "bg-lunar-healthy/15 hover:bg-lunar-healthy/25 text-lunar-healthy border-lunar-healthy/30"
            }`}
          >
            {isActionLoading ? (
              <RotateCcw className="w-3 h-3 animate-spin" />
            ) : item.isEnabled ? (
              <>
                <Power className="w-3 h-3" />
                <span>Disable</span>
              </>
            ) : (
              <>
                <RotateCcw className="w-3 h-3" />
                <span>Restore</span>
              </>
            )}
          </button>
        )}
      </div>
    </div>
  );
};
