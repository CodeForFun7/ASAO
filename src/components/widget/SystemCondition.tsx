import React from "react";
import type { SystemConditionState } from "../../types/widget";

interface SystemConditionProps {
  condition: SystemConditionState;
  reason: string;
}

export const SystemCondition: React.FC<SystemConditionProps> = ({
  condition,
  reason,
}) => {
  const config =
    condition === "ATTENTION"
      ? {
          label: "ATTENTION",
          dot: "bg-lunar-critical",
          text: "text-lunar-critical",
          pill: "bg-lunar-critical/15 border-lunar-critical/30",
        }
      : condition === "ELEVATED"
      ? {
          label: "ELEVATED",
          dot: "bg-lunar-warning",
          text: "text-lunar-warning",
          pill: "bg-lunar-warning/15 border-lunar-warning/30",
        }
      : {
          label: "GOOD",
          dot: "bg-lunar-healthy",
          text: "text-lunar-healthy",
          pill: "bg-lunar-healthy/15 border-lunar-healthy/30",
        };

  return (
    <div
      data-tauri-drag-region
      className="flex items-center justify-between px-3 py-2 rounded-lg bg-lunar-bg/60 border border-lunar-border/80"
    >
      <div className="min-w-0 pr-2">
        <div className="text-[10px] uppercase tracking-[0.14em] text-lunar-muted">
          System Condition
        </div>
        <div className="text-[11px] text-lunar-text-sec truncate mt-0.5">
          {reason}
        </div>
      </div>

      <div
        className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-md border text-xs font-semibold shrink-0 ${config.pill} ${config.text}`}
      >
        <span className={`w-2 h-2 rounded-full ${config.dot}`} />
        <span>{config.label}</span>
      </div>
    </div>
  );
};
