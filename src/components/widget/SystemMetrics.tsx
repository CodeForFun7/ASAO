import React from "react";
import type { WidgetHistorySample } from "../../types/widget";

interface SystemMetricsProps {
  cpuUsage: number;
  memoryUsage: number;
  memoryUsedBytes?: number;
  memoryTotalBytes?: number;
  history?: WidgetHistorySample[];
}

export const SystemMetrics: React.FC<SystemMetricsProps> = ({
  cpuUsage,
  memoryUsage,
}) => {
  return (
    <div className="grid grid-cols-2 gap-2.5 shrink-0">
      {/* CPU Card */}
      <div className="p-2.5 rounded-lg bg-lunar-surface/55 backdrop-blur-sm border border-lunar-border/80 flex flex-col justify-between">
        <div className="flex items-baseline justify-between">
          <span className="text-[10px] uppercase tracking-[0.14em] text-lunar-text-sec font-medium">
            CPU
          </span>
          <span className="text-base font-semibold text-lunar-white">
            {cpuUsage.toFixed(0)}%
          </span>
        </div>

        {/* Utilization Bar (Monotonic) */}
        <div className="mt-2 h-1.5 w-full bg-lunar-bg/60 rounded-sm overflow-hidden border border-lunar-border/80">
          <div
            className="h-full transition-all duration-300 bg-lunar-white"
            style={{ width: `${Math.max(3, Math.min(100, cpuUsage))}%` }}
          />
        </div>
      </div>

      {/* RAM Card */}
      <div className="p-2.5 rounded-lg bg-lunar-surface/55 backdrop-blur-sm border border-lunar-border/80 flex flex-col justify-between">
        <div className="flex items-baseline justify-between">
          <span className="text-[10px] uppercase tracking-[0.14em] text-lunar-text-sec font-medium">
            RAM
          </span>
          <span className="text-base font-semibold text-lunar-white">
            {memoryUsage.toFixed(0)}%
          </span>
        </div>

        {/* Utilization Bar (Monotonic) */}
        <div className="mt-2 h-1.5 w-full bg-lunar-bg/60 rounded-sm overflow-hidden border border-lunar-border/80">
          <div
            className="h-full transition-all duration-300 bg-lunar-white"
            style={{ width: `${Math.max(3, Math.min(100, memoryUsage))}%` }}
          />
        </div>
      </div>
    </div>
  );
};
