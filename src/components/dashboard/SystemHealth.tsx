import React from "react";
import type { SystemMetrics } from "../../types/process";
import { formatBytes } from "../../services/tauri";

interface SystemHealthProps {
  metrics: SystemMetrics;
  onSelectCpu: () => void;
  onSelectMemory: () => void;
  onSelectAttention: () => void;
}

export const SystemHealth: React.FC<SystemHealthProps> = ({
  metrics,
  onSelectCpu,
  onSelectMemory,
  onSelectAttention,
}) => {
  const attentionRatio =
    metrics.totalProcesses > 0
      ? Math.min(100, (metrics.attentionProcesses / Math.max(20, metrics.totalProcesses * 0.2)) * 100)
      : 0;

  return (
    <section className="rounded-lg bg-lunar-surface border border-lunar-border p-5">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-text-sec">
          System Health
        </h2>
        <span className="text-[11px] font-mono text-lunar-muted">
          REAL-TIME TELEMETRY GAUGES
        </span>
      </div>

      <div className="space-y-4">
        {/* CPU Bar */}
        <div
          onClick={onSelectCpu}
          className="group grid grid-cols-12 items-center gap-4 py-1.5 px-2 -mx-2 rounded hover:bg-lunar-surface-2 transition-colors cursor-pointer"
          title="Click to inspect processes sorted by CPU usage"
        >
          <div className="col-span-3 sm:col-span-2 text-xs font-medium text-lunar-text">
            CPU
          </div>
          <div className="col-span-6 sm:col-span-7">
            <div className="h-2 w-full bg-lunar-bg rounded-sm overflow-hidden border border-lunar-border p-[1px]">
              <div
                className={`h-full transition-all duration-300 rounded-[1px] ${
                  metrics.cpuUsagePercent > 80
                    ? "bg-lunar-critical"
                    : metrics.cpuUsagePercent > 50
                    ? "bg-lunar-warning"
                    : "bg-lunar-white"
                }`}
                style={{ width: `${Math.max(2, Math.min(100, metrics.cpuUsagePercent))}%` }}
              />
            </div>
          </div>
          <div className="col-span-3 text-right font-mono text-xs text-lunar-text-sec group-hover:text-lunar-white">
            {metrics.cpuUsagePercent.toFixed(0)}%
          </div>
        </div>

        {/* Memory Bar */}
        <div
          onClick={onSelectMemory}
          className="group grid grid-cols-12 items-center gap-4 py-1.5 px-2 -mx-2 rounded hover:bg-lunar-surface-2 transition-colors cursor-pointer"
          title="Click to inspect processes sorted by Memory usage"
        >
          <div className="col-span-3 sm:col-span-2 text-xs font-medium text-lunar-text">
            Memory
          </div>
          <div className="col-span-6 sm:col-span-7">
            <div className="h-2 w-full bg-lunar-bg rounded-sm overflow-hidden border border-lunar-border p-[1px]">
              <div
                className={`h-full transition-all duration-300 rounded-[1px] ${
                  metrics.memoryUsagePercent > 85
                    ? "bg-lunar-critical"
                    : metrics.memoryUsagePercent > 70
                    ? "bg-lunar-warning"
                    : "bg-lunar-text"
                }`}
                style={{
                  width: `${Math.max(2, Math.min(100, metrics.memoryUsagePercent))}%`,
                }}
              />
            </div>
          </div>
          <div className="col-span-3 text-right font-mono text-xs text-lunar-text-sec group-hover:text-lunar-white">
            {metrics.memoryUsagePercent.toFixed(0)}%{" "}
            <span className="hidden md:inline text-[11px] text-lunar-muted">
              ({formatBytes(metrics.memoryUsedBytes)} /{" "}
              {formatBytes(metrics.memoryTotalBytes)})
            </span>
          </div>
        </div>

        {/* Processes Bar */}
        <div
          onClick={onSelectAttention}
          className="group grid grid-cols-12 items-center gap-4 py-1.5 px-2 -mx-2 rounded hover:bg-lunar-surface-2 transition-colors cursor-pointer"
          title="Click to filter processes requiring attention"
        >
          <div className="col-span-3 sm:col-span-2 text-xs font-medium text-lunar-text">
            Processes
          </div>
          <div className="col-span-6 sm:col-span-7">
            <div className="h-2 w-full bg-lunar-bg rounded-sm overflow-hidden border border-lunar-border p-[1px]">
              <div
                className={`h-full transition-all duration-300 rounded-[1px] ${
                  metrics.attentionProcesses > 0
                    ? "bg-lunar-warning"
                    : "bg-lunar-healthy"
                }`}
                style={{ width: `${Math.max(4, attentionRatio)}%` }}
              />
            </div>
          </div>
          <div className="col-span-3 text-right font-mono text-xs text-lunar-text-sec group-hover:text-lunar-white">
            {metrics.attentionProcesses} require attention
          </div>
        </div>
      </div>
    </section>
  );
};
