import React from "react";
import { ArrowUpRight } from "lucide-react";
import type { SystemMetrics } from "../../types/process";

interface SystemStrainPanelProps {
  metrics: SystemMetrics;
  onSelectProcesses: () => void;
}

export const SystemStrainPanel: React.FC<SystemStrainPanelProps> = ({
  metrics,
  onSelectProcesses,
}) => {
  const gpuPercent = metrics.gpuUsagePercent ?? 0;
  const strainPercent = Math.min(
    100,
    Math.max(
      1,
      Math.round(
        metrics.cpuUsagePercent * 0.45 +
          metrics.memoryUsagePercent * 0.4 +
          gpuPercent * 0.15
      )
    )
  );

  const statusConfig =
    metrics.systemStatus === "critical"
      ? {
          label: "Strained",
          sub: "Elevated system load",
          dot: "bg-lunar-critical",
          stroke: "#C47A7A",
        }
      : metrics.systemStatus === "warning"
      ? {
          label: "Moderate Load",
          sub: "Resource thresholds active",
          dot: "bg-lunar-warning",
          stroke: "#C9A66B",
        }
      : {
          label: "Healthy",
          sub: "Nominal operating parameters",
          dot: "bg-lunar-healthy",
          stroke: "#9BAE9F",
        };

  // Sleep-Time style open-bottom semi-circle / 240° gauge arc
  const radius = 42;
  const circumference = 2 * Math.PI * radius;
  const arcAngle = 245; // 245° sweep with open bottom (like Sleep Time in ref)
  const arcLength = (arcAngle / 360) * circumference;
  const filledLength = (strainPercent / 100) * arcLength;

  // Healthy ratio for the Processes bar
  const nominalProcesses = Math.max(
    0,
    metrics.totalProcesses - metrics.highResourceProcesses - metrics.attentionProcesses
  );
  const nominalRatioPercent =
    metrics.totalProcesses > 0
      ? Math.min(
          100,
          Math.max(12, Math.round((nominalProcesses / metrics.totalProcesses) * 100))
        )
      : 100;

  return (
    <div
      onClick={onSelectProcesses}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelectProcesses();
        }
      }}
      className="group h-full rounded-xl lunar-glass-card-interactive p-6 flex flex-col justify-between cursor-pointer"
    >
      {/* Top: System Strain Header & Status */}
      <div className="flex items-start justify-between gap-3">
        <div>
          <span className="text-sm font-semibold text-lunar-white tracking-tight block">
            System Strain &amp; Processes
          </span>
          <p className="text-xs text-lunar-text-sec mt-1">
            {statusConfig.sub}
          </p>
        </div>

        <div className="flex items-center gap-2 px-2.5 py-1 rounded-full bg-lunar-bg/70 border border-lunar-border shrink-0">
          <span
            className={`w-2 h-2 rounded-full shrink-0 ${statusConfig.dot}`}
          />
          <span className="text-xs font-medium text-lunar-text">
            {statusConfig.label}
          </span>
        </div>
      </div>

      {/* Middle: Centered Semi-Circle Strain Gauge Chart */}
      <div className="my-4 flex flex-col items-center justify-center">
        <div className="relative w-40 h-40 flex items-center justify-center">
          <svg viewBox="0 0 104 104" className="w-full h-full">
            {/* Background Track Arc */}
            <circle
              cx="52"
              cy="52"
              r={radius}
              fill="none"
              stroke="#262C34"
              strokeWidth="8.5"
              strokeLinecap="round"
              strokeDasharray={`${arcLength} ${circumference}`}
              transform="rotate(147.5 52 52)"
            />
            {/* Active Strain Arc */}
            <circle
              cx="52"
              cy="52"
              r={radius}
              fill="none"
              stroke={statusConfig.stroke}
              strokeWidth="8.5"
              strokeLinecap="round"
              strokeDasharray={`${filledLength} ${circumference}`}
              transform="rotate(147.5 52 52)"
              className="transition-all duration-500 ease-out"
            />
          </svg>
          <div className="absolute inset-0 flex flex-col items-center justify-center text-center">
            <span className="text-2xl font-semibold text-lunar-white font-mono leading-none">
              {strainPercent}%
            </span>
            <span className="text-[10px] font-mono uppercase tracking-wider text-lunar-muted mt-1.5">
              SYSTEM STRAIN
            </span>
          </div>
        </div>
      </div>

      {/* Bottom: Merged Processes Overview */}
      <div className="pt-4">
        <div className="flex items-end justify-between gap-2">
          <div>
            <div className="flex items-center gap-1.5">
              <span className="text-xs font-medium text-lunar-text-sec group-hover:text-lunar-white transition-colors">
                Active Processes
              </span>
              <ArrowUpRight className="w-3.5 h-3.5 text-lunar-muted group-hover:text-lunar-white transition-colors" />
            </div>
            <div className="text-2xl font-semibold text-lunar-white font-mono tracking-tight mt-0.5">
              {metrics.totalProcesses}
            </div>
          </div>

          <div className="text-right font-mono">
            <span
              className={`text-xs font-medium block ${
                metrics.attentionProcesses > 0
                  ? "text-lunar-warning"
                  : "text-lunar-healthy"
              }`}
            >
              {metrics.attentionProcesses} require attention
            </span>
            <span className="text-[11px] text-lunar-muted">
              {nominalRatioPercent}% nominal
            </span>
          </div>
        </div>

        <div className="flex items-center justify-between text-[11px] font-mono text-lunar-text-sec mt-2">
          <span>Protected: {metrics.protectedProcesses}</span>
          <span>High Load: {metrics.highResourceProcesses}</span>
        </div>
      </div>
    </div>
  );
};
