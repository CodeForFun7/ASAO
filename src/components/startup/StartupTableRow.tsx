import React from "react";
import {
  Cpu,
  Database,
  FolderSync,
  HelpCircle,
  Layers,
  ShieldAlert,
  ShieldCheck,
  Terminal,
} from "lucide-react";
import type { StartupGroup, StartupItem } from "../../types/startup";
import { SOURCE_LABELS } from "../../types/startup";
import { formatBytes } from "../../services/tauri";

interface StartupTableRowProps {
  item: StartupItem;
  isSelected: boolean;
  onSelect: (id: string) => void;
}

function getGroupIcon(group: StartupGroup) {
  switch (group) {
    case "Registry Startup":
      return <Terminal className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "Startup Folder":
      return <FolderSync className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "Scheduled Tasks":
      return <Layers className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "Windows Services":
      return <Database className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "Winlogon / System Startup":
      return <ShieldCheck className="w-3.5 h-3.5 text-lunar-text-sec" />;
    default:
      return <Cpu className="w-3.5 h-3.5 text-lunar-text-sec" />;
  }
}

function getImpactLevel(item: StartupItem) {
  switch (item.impact) {
    case "high":
      return {
        label: "High",
        textColor: "text-lunar-critical",
      };
    case "medium":
      return {
        label: "Medium",
        textColor: "text-lunar-warning",
      };
    case "low":
    default:
      return {
        label: "Low",
        textColor: "text-lunar-healthy",
      };
  }
}

function getRecommendationIcon(recommendation: string) {
  switch (recommendation) {
    case "disable":
      return {
        icon: ShieldAlert,
        color: "text-lunar-critical",
      };
    case "keep":
      return {
        icon: ShieldCheck,
        color: "text-lunar-healthy",
      };
    case "investigate":
    default:
      return {
        icon: HelpCircle,
        color: "text-lunar-warning",
      };
  }
}

export const StartupTableRow: React.FC<StartupTableRowProps> = React.memo(
  ({ item, isSelected, onSelect }) => {
    const impactMeta = getImpactLevel(item);
    const sourceLabel = SOURCE_LABELS[item.source] ?? item.source;
    const { icon: RecIcon, color: recColor } = getRecommendationIcon(
      item.recommendation
    );

    return (
      <div
        role="button"
        tabIndex={0}
        onClick={() => onSelect(item.id)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect(item.id);
          }
        }}
        className={`grid grid-cols-12 items-center gap-3 px-4 py-2.5 transition-colors cursor-pointer select-none text-left ${
          isSelected
            ? "bg-lunar-surface-2 border-l-2 border-l-lunar-white"
            : "hover:bg-lunar-surface/60 border-l-2 border-l-transparent"
        }`}
      >
        {/* 1. Application Name & Identity */}
        <div className="col-span-4 sm:col-span-3 flex items-center gap-2.5 min-w-0">
          <div className="w-6 h-6 rounded bg-lunar-surface-2 border border-lunar-border flex items-center justify-center shrink-0">
            {getGroupIcon(item.startupGroup)}
          </div>

          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span className="text-xs font-medium text-lunar-white truncate">
                {item.name}
              </span>

              {/* Recommendation Icon */}
              <span
                title={`Recommendation: ${item.recommendation.toUpperCase()}`}
                className={`${recColor} shrink-0 cursor-help`}
              >
                <RecIcon className="w-3 h-3" />
              </span>
            </div>

            <div className="text-[11px] text-lunar-muted truncate font-mono mt-0.5">
              {item.publisher ?? item.executablePath ?? sourceLabel}
            </div>
          </div>
        </div>

        {/* 2. Source / Mechanism */}
        <div className="hidden sm:flex sm:col-span-2 items-center min-w-0">
          <span className="text-xs text-lunar-text-sec truncate font-mono">
            {sourceLabel}
          </span>
        </div>

        {/* 3. CPU */}
        <div className="col-span-2 sm:col-span-1 text-right font-mono">
          <div
            className={`text-xs ${
              item.bootCpuMs >= 200
                ? "text-lunar-critical font-semibold"
                : item.bootCpuMs >= 50
                ? "text-lunar-warning"
                : "text-lunar-text-sec"
            }`}
          >
            {item.bootCpuMs}ms
          </div>
          <div className="text-[10px] text-lunar-muted">CPU</div>
        </div>

        {/* 4. Memory */}
        <div className="col-span-2 sm:col-span-2 lg:col-span-1 text-right font-mono">
          <div
            className={`text-xs ${
              item.memoryBytes >= 400 * 1024 * 1024
                ? "text-lunar-warning font-semibold"
                : item.memoryBytes >= 100 * 1024 * 1024
                ? "text-lunar-white"
                : "text-lunar-text-sec"
            }`}
          >
            {formatBytes(item.memoryBytes)}
          </div>
          <div className="text-[10px] text-lunar-muted">RAM</div>
        </div>

        {/* 5. Disk */}
        <div className="hidden lg:block lg:col-span-1 text-right font-mono">
          <div className="text-xs text-lunar-text-sec">
            {formatBytes(item.bootDiskBytes)}
          </div>
          <div className="text-[10px] text-lunar-muted">Disk</div>
        </div>

        {/* 6. Delay Duration */}
        <div className="hidden lg:block lg:col-span-1 text-right font-mono">
          <div className="text-xs text-lunar-text-sec">
            ~{item.bootDurationMs}ms
          </div>
          <div className="text-[10px] text-lunar-muted">Delay</div>
        </div>

        {/* 7. Impact (Clean text without box or tint) */}
        <div className="col-span-2 sm:col-span-2 lg:col-span-1 text-right font-mono">
          <span className={`text-xs font-medium ${impactMeta.textColor}`}>
            {impactMeta.label}
          </span>
        </div>

        {/* 8. Status (Clean text without box or tint) */}
        <div className="col-span-2 sm:col-span-2 lg:col-span-2 text-right font-mono">
          {item.isCurrentlyRunning ? (
            <span
              className="text-xs font-medium text-lunar-healthy cursor-help"
              title={
                item.pid
                  ? `PID: ${item.pid} (Running, ${formatBytes(item.memoryBytes)} RAM)`
                  : `Running (${formatBytes(item.memoryBytes)} RAM)`
              }
            >
              Running
            </span>
          ) : (
            <span
              className="text-xs text-lunar-muted cursor-help"
              title="Process is not currently running"
            >
              Inactive
            </span>
          )}
        </div>
      </div>
    );
  }
);
