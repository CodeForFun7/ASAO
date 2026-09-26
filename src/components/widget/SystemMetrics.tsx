import React, { useMemo } from "react";
import type { WidgetHistorySample } from "../../types/widget";
import { formatBytes } from "../../services/tauri";
import { ResourceMiniChart } from "./ResourceMiniChart";

interface SystemMetricsProps {
  cpuUsage: number;
  memoryUsage: number;
  memoryUsedBytes: number;
  memoryTotalBytes: number;
  history: WidgetHistorySample[];
}

export const SystemMetrics: React.FC<SystemMetricsProps> = ({
  cpuUsage,
  memoryUsage,
  memoryUsedBytes,
  memoryTotalBytes,
  history,
}) => {
  const cpuValues = useMemo(() => history.map((h) => h.cpuUsage), [history]);
  const memValues = useMemo(() => history.map((h) => h.memoryUsage), [history]);

  return (
    <div className="grid grid-cols-2 gap-2.5">
      {/* CPU Card */}
      <div className="p-2.5 rounded-lg bg-lunar-bg/60 border border-lunar-border/80 flex flex-col justify-between">
        <div className="flex items-baseline justify-between">
          <span className="text-[10px] uppercase tracking-[0.14em] text-lunar-text-sec font-medium">
            CPU
          </span>
          <span className="text-base font-semibold text-lunar-white">
            {cpuUsage.toFixed(0)}%
          </span>
        </div>

        {/* Utilization Bar */}
        <div className="mt-1.5 h-1.5 w-full bg-lunar-surface rounded-sm overflow-hidden border border-lunar-border">
          <div
            className={`h-full transition-all duration-300 ${
              cpuUsage >= 80
                ? "bg-lunar-critical"
                : cpuUsage >= 50
                ? "bg-lunar-warning"
                : "bg-lunar-white"
            }`}
            style={{ width: `${Math.max(3, Math.min(100, cpuUsage))}%` }}
          />
        </div>

        {/* Subtle Real-Time Activity Curve */}
        <ResourceMiniChart
          values={cpuValues}
          strokeColor="#E8EAED"
          gradientId="widgetCpuMini"
        />
      </div>

      {/* RAM Card */}
      <div className="p-2.5 rounded-lg bg-lunar-bg/60 border border-lunar-border/80 flex flex-col justify-between">
        <div className="flex items-baseline justify-between">
          <span className="text-[10px] uppercase tracking-[0.14em] text-lunar-text-sec font-medium">
            RAM
          </span>
          <span className="text-base font-semibold text-lunar-white">
            {memoryUsage.toFixed(0)}%
          </span>
        </div>

        {/* Utilization Bar */}
        <div className="mt-1.5 h-1.5 w-full bg-lunar-surface rounded-sm overflow-hidden border border-lunar-border">
          <div
            className={`h-full transition-all duration-300 ${
              memoryUsage >= 88
                ? "bg-lunar-critical"
                : memoryUsage >= 72
                ? "bg-lunar-warning"
                : "bg-lunar-healthy"
            }`}
            style={{ width: `${Math.max(3, Math.min(100, memoryUsage))}%` }}
          />
        </div>

        {/* Subtle Real-Time Activity Curve + GB Readout */}
        <ResourceMiniChart
          values={memValues}
          strokeColor="#9BAE9F"
          gradientId="widgetRamMini"
        />
        <div className="text-[9px] text-lunar-muted text-right -mt-0.5">
          {formatBytes(memoryUsedBytes)} / {formatBytes(memoryTotalBytes)}
        </div>
      </div>
    </div>
  );
};
