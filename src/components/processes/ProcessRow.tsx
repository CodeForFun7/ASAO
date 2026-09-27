import React from "react";
import {
  Shield,
  Cpu,
  Gamepad2,
  Code2,
  Briefcase,
  MessageSquare,
  Globe,
  HelpCircle,
} from "lucide-react";
import {
  CATEGORY_METADATA,
  type ProcessCategory,
  type ProcessInfo,
} from "../../types/process";
import { formatBytes, formatRate } from "../../services/tauri";

interface ProcessRowProps {
  process: ProcessInfo;
  isSelected: boolean;
  onSelect: (pid: number) => void;
}

function getResourceLevel(process: ProcessInfo): {
  label: "High" | "Moderate" | "Low";
  dotColor: string;
  badgeBg: string;
  badgeText: string;
  badgeBorder: string;
} {
  if (
    process.cpuPercent >= 12 ||
    process.memoryBytes >= 1500 * 1024 * 1024 ||
    process.status === "high-resource" ||
    process.status === "attention"
  ) {
    return {
      label: "High",
      dotColor: "bg-lunar-critical",
      badgeBg: "bg-lunar-critical/10",
      badgeText: "text-lunar-critical",
      badgeBorder: "border-lunar-critical/30",
    };
  }

  if (process.cpuPercent >= 5 || process.memoryBytes >= 500 * 1024 * 1024) {
    return {
      label: "Moderate",
      dotColor: "bg-lunar-warning",
      badgeBg: "bg-lunar-warning/10",
      badgeText: "text-lunar-warning",
      badgeBorder: "border-lunar-warning/30",
    };
  }

  return {
    label: "Low",
    dotColor: "bg-lunar-healthy",
    badgeBg: "bg-lunar-healthy/10",
    badgeText: "text-lunar-healthy",
    badgeBorder: "border-lunar-healthy/30",
  };
}

function getCategoryIcon(category: ProcessCategory) {
  switch (category) {
    case "windows-core":
      return <Shield className="w-3.5 h-3.5 text-lunar-ai" />;
    case "drivers":
      return <Cpu className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "gaming":
      return <Gamepad2 className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "development":
      return <Code2 className="w-3.5 h-3.5 text-lunar-white" />;
    case "productivity":
      return <Briefcase className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "communication":
      return <MessageSquare className="w-3.5 h-3.5 text-lunar-text-sec" />;
    case "browser":
      return <Globe className="w-3.5 h-3.5 text-lunar-text-sec" />;
    default:
      return <HelpCircle className="w-3.5 h-3.5 text-lunar-muted" />;
  }
}

export const ProcessRow: React.FC<ProcessRowProps> = React.memo(
  ({ process, isSelected, onSelect }) => {
    const resourceMeta = getResourceLevel(process);
    const categoryLabel =
      CATEGORY_METADATA[process.category]?.label ?? "Unknown";

    return (
      <div
        onClick={() => onSelect(process.pid)}
        role="row"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect(process.pid);
          }
        }}
        className={`grid grid-cols-12 items-center gap-3 px-4 py-3 border-b border-lunar-border/60 transition-colors cursor-pointer ${
          isSelected
            ? "bg-lunar-elevated border-l-2 border-l-lunar-white"
            : "hover:bg-lunar-surface-2"
        }`}
      >
        {/* Process Name & Publisher */}
        <div className="col-span-4 sm:col-span-3 flex items-center gap-3 min-w-0">
          <div className="relative flex items-center justify-center w-7 h-7 rounded bg-lunar-bg border border-lunar-border shrink-0">
            {getCategoryIcon(process.category)}
            <span
              className={`absolute -bottom-0.5 -right-0.5 w-2 h-2 rounded-full border border-lunar-bg ${resourceMeta.dotColor}`}
            />
          </div>

          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span className="text-xs font-medium text-lunar-white truncate">
                {process.name}
              </span>
              {process.isRestricted && (
                <span
                  className="text-[9px] font-mono px-1 rounded bg-lunar-bg text-lunar-muted border border-lunar-border"
                  title="Restricted Windows process under normal user privileges"
                >
                  RESTRICTED
                </span>
              )}
            </div>
            <div className="text-[11px] text-lunar-muted truncate font-mono">
              {process.publisher ?? process.description ?? `PID ${process.pid}`}
            </div>
          </div>
        </div>

        {/* Category */}
        <div className="hidden sm:flex sm:col-span-2 items-center min-w-0">
          <span className="text-xs text-lunar-text-sec truncate">
            {categoryLabel}
          </span>
        </div>

        {/* CPU */}
        <div className="col-span-2 sm:col-span-1 text-right font-mono">
          <div
            className={`text-xs ${
              process.cpuPercent >= 25
                ? "text-lunar-warning font-semibold"
                : process.cpuPercent >= 5
                ? "text-lunar-white"
                : "text-lunar-text-sec"
            }`}
          >
            {process.cpuPercent.toFixed(1)}%
          </div>
          <div className="text-[10px] text-lunar-muted">CPU</div>
        </div>

        {/* Memory */}
        <div className="col-span-2 sm:col-span-2 text-right font-mono">
          <div
            className={`text-xs ${
              process.memoryBytes >= 800 * 1024 * 1024
                ? "text-lunar-warning font-semibold"
                : process.memoryBytes >= 300 * 1024 * 1024
                ? "text-lunar-white"
                : "text-lunar-text-sec"
            }`}
          >
            {process.isRestricted && process.memoryBytes === 0
              ? "Restricted"
              : formatBytes(process.memoryBytes)}
          </div>
          <div className="text-[10px] text-lunar-muted">RAM</div>
        </div>

        {/* Disk */}
        <div className="hidden lg:block lg:col-span-1 text-right font-mono">
          <div className="text-xs text-lunar-text-sec">
            {formatRate(process.diskBytesPerSec)}
          </div>
          <div className="text-[10px] text-lunar-muted">Disk</div>
        </div>

        {/* Network */}
        <div className="hidden lg:block lg:col-span-1 text-right font-mono">
          <div className="text-xs text-lunar-text-sec">
            {formatRate(process.networkBytesPerSec)}
          </div>
          <div className="text-[10px] text-lunar-muted">Net</div>
        </div>

        {/* Resource */}
        <div className="col-span-4 sm:col-span-4 lg:col-span-2 flex justify-end">
          <span
            className={`inline-flex items-center gap-1.5 rounded border px-2 py-0.5 text-[11px] font-mono ${resourceMeta.badgeBg} ${resourceMeta.badgeText} ${resourceMeta.badgeBorder}`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full shrink-0 ${resourceMeta.dotColor}`}
            />
            <span className="truncate">{resourceMeta.label}</span>
          </span>
        </div>
      </div>
    );
  }
);
