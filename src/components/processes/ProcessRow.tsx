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
  STATUS_METADATA,
  type ProcessCategory,
  type ProcessInfo,
} from "../../types/process";
import { formatBytes, formatRate } from "../../services/tauri";
import { ProcessStatusBadge } from "./ProcessStatusBadge";

interface ProcessRowProps {
  process: ProcessInfo;
  isSelected: boolean;
  onSelect: (pid: number) => void;
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
    const statusMeta =
      STATUS_METADATA[process.status] ?? STATUS_METADATA.normal;
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
              className={`absolute -bottom-0.5 -right-0.5 w-2 h-2 rounded-full border border-lunar-bg ${statusMeta.dotColor}`}
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

        {/* Status */}
        <div className="col-span-4 sm:col-span-4 lg:col-span-2 flex justify-end">
          <ProcessStatusBadge status={process.status} compact />
        </div>
      </div>
    );
  }
);
