import React from "react";
import type { SystemConditionState } from "../../types/widget";

interface SystemConditionProps {
  condition: SystemConditionState;
  reason: string;
}

export const SystemCondition: React.FC<SystemConditionProps> = ({
  condition,
}) => {
  const config =
    condition === "ATTENTION"
      ? {
          label: "HIGH",
          text: "text-lunar-critical",
        }
      : condition === "ELEVATED"
      ? {
          label: "MODERATE",
          text: "text-lunar-warning",
        }
      : {
          label: "LOW",
          text: "text-lunar-healthy",
        };

  return (
    <div
      data-tauri-drag-region
      className="flex items-center justify-between px-3 py-2.5 rounded-lg bg-lunar-surface/55 backdrop-blur-sm border border-lunar-border/80 shrink-0"
    >
      <div className="text-[10px] uppercase tracking-[0.14em] text-lunar-muted font-medium">
        Strain
      </div>

      <div
        className={`inline-flex items-center justify-center text-center text-xs font-semibold tracking-wider shrink-0 ${config.text}`}
      >
        {config.label}
      </div>
    </div>
  );
};
