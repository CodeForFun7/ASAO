import React from "react";
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
        metrics.systemStrainPercent ??
          metrics.cpuUsagePercent * 0.45 +
            metrics.memoryUsagePercent * 0.4 +
            gpuPercent * 0.15
      )
    )
  );

  const activeTier: "low" | "moderate" | "high" =
    metrics.systemStatus === "critical"
      ? "high"
      : metrics.systemStatus === "warning"
      ? "moderate"
      : "low";

  const strokeColor =
    activeTier === "high"
      ? "#EF4444"
      : activeTier === "moderate"
      ? "#F59E0B"
      : "#22C55E";

  // Open-bottom semi-circle / 245° gauge arc
  const radius = 42;
  const circumference = 2 * Math.PI * radius;
  const arcAngle = 245;
  const arcLength = (arcAngle / 360) * circumference;
  const filledLength = (strainPercent / 100) * arcLength;

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
      {/* Top: Centered System Strain and Processes Title */}
      <div className="text-center flex flex-col items-center">
        <span className="text-sm font-semibold text-lunar-white tracking-tight block">
          System Strain and Processes
        </span>
      </div>

      {/* Middle: Centered Semi-Circle Strain Gauge Chart + Low / Moderate / High Legend Row */}
      <div className="my-4 flex flex-col items-center justify-center">
        <div className="relative w-44 h-44 flex items-center justify-center">
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
              stroke={strokeColor}
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

        {/* Low / Moderate / High Horizontal Row */}
        <div className="mt-3 w-full flex items-center justify-around px-2">
          <div
            className={`flex items-center gap-2 px-2.5 py-1 rounded-full transition-colors ${
              activeTier === "low"
                ? "bg-lunar-bg/80 border border-lunar-border text-lunar-white"
                : "text-lunar-text-sec opacity-60"
            }`}
          >
            <span className="w-2.5 h-2.5 rounded-full bg-[#22C55E] shrink-0" />
            <span className="text-xs font-medium">Low</span>
          </div>

          <div
            className={`flex items-center gap-2 px-2.5 py-1 rounded-full transition-colors ${
              activeTier === "moderate"
                ? "bg-lunar-bg/80 border border-lunar-border text-lunar-white"
                : "text-lunar-text-sec opacity-60"
            }`}
          >
            <span className="w-2.5 h-2.5 rounded-full bg-[#F59E0B] shrink-0" />
            <span className="text-xs font-medium">Moderate</span>
          </div>

          <div
            className={`flex items-center gap-2 px-2.5 py-1 rounded-full transition-colors ${
              activeTier === "high"
                ? "bg-lunar-bg/80 border border-lunar-border text-lunar-white"
                : "text-lunar-text-sec opacity-60"
            }`}
          >
            <span className="w-2.5 h-2.5 rounded-full bg-[#EF4444] shrink-0" />
            <span className="text-xs font-medium">High</span>
          </div>
        </div>
      </div>

      {/* Bottom: Active Processes & High Load Key-Value Rows */}
      <div className="space-y-2.5 pt-2">
        <div className="flex items-center justify-between text-sm">
          <span className="text-lunar-text-sec font-medium">
            Active Processes:
          </span>
          <span className="font-mono font-semibold text-lunar-white">
            {metrics.totalProcesses}
          </span>
        </div>

        <div className="flex items-center justify-between text-sm">
          <span className="text-lunar-text-sec font-medium">High Load:</span>
          <span className="font-mono font-semibold text-lunar-white">
            {metrics.highResourceProcesses}
          </span>
        </div>
      </div>
    </div>
  );
};

