import React from "react";
import { Cpu, Activity, HardDrive, Rocket } from "lucide-react";
import type { DiagnosticVisualData } from "../../types/agent";

interface MetricPillGroupProps {
  data: DiagnosticVisualData;
}

export const MetricPillGroup: React.FC<MetricPillGroupProps> = ({ data }) => {
  const overview = data.overview;
  const startup = data.startup;
  const storage = data.storage;
  const primaryDrive = storage?.drives?.[0];

  const visiblePills = data.visiblePills;
  const showCpu = (!visiblePills || visiblePills.includes("cpu")) && Boolean(overview);
  const showRam = (!visiblePills || visiblePills.includes("ram")) && Boolean(overview);
  const showDisk = (!visiblePills || visiblePills.includes("disk")) && Boolean(primaryDrive || overview);
  const showStartup = (!visiblePills || visiblePills.includes("startup")) && Boolean(startup);

  const activePillsCount = [showCpu, showRam, showDisk, showStartup].filter(Boolean).length;
  if (activePillsCount === 0) return null;

  const cpuVal = overview?.cpuUsagePercent ?? 0;
  const ramVal = overview?.memoryUsagePercent ?? 0;
  const diskVal = primaryDrive?.usagePercentage ?? 0;
  const startupCount = startup?.totalStartupItems ?? 0;
  const highImpactCount = startup?.highImpactCount ?? 0;

  const getMetricBadge = (val: number, highThreshold = 80, warningThreshold = 60) => {
    if (val >= highThreshold) {
      return { label: "High", className: "bg-lunar-white text-lunar-bg font-semibold" };
    }
    if (val >= warningThreshold) {
      return { label: "Elevated", className: "bg-lunar-elevated text-lunar-white border border-lunar-border" };
    }
    return { label: "Normal", className: "bg-lunar-surface-2 text-lunar-text-sec border border-lunar-border" };
  };

  const getHeaderTitle = () => {
    if (showStartup && !showCpu && !showDisk) return "Startup Telemetry";
    if (showDisk && !showCpu && !showStartup) return "Storage Telemetry";
    if ((showCpu || showRam) && !showStartup && !showDisk) return "Active Load Telemetry";
    return "System Telemetry";
  };

  const gridColsClass =
    activePillsCount === 1
      ? "grid grid-cols-1 max-w-xs gap-2.5"
      : activePillsCount === 2
      ? "grid grid-cols-1 sm:grid-cols-2 gap-2.5"
      : activePillsCount === 3
      ? "grid grid-cols-1 sm:grid-cols-3 gap-2.5"
      : "grid grid-cols-2 sm:grid-cols-4 gap-2.5";

  return (
    <div className="my-3">
      <div className="text-[11px] font-medium tracking-wide text-lunar-text-sec mb-2 flex items-center justify-between">
        <span>{getHeaderTitle()}</span>
        {overview?.condition && (showCpu || showRam) && (
          <span className="flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-mono bg-lunar-elevated text-lunar-white border border-lunar-border">
            <span className="w-1.5 h-1.5 rounded-full bg-lunar-white animate-pulse" />
            <span className="capitalize">{overview.condition}</span>
          </span>
        )}
      </div>

      <div className={gridColsClass}>
        {/* CPU Pill */}
        {showCpu && overview && (
          <div className="p-3 rounded-xl bg-lunar-surface border border-lunar-border hover:bg-lunar-surface-2 transition-all flex flex-col justify-between shadow-xs">
            <div className="flex items-center justify-between text-lunar-text-sec text-[10px] mb-1">
              <span className="flex items-center gap-1.5 font-medium text-lunar-text">
                <Cpu className="w-3.5 h-3.5 text-lunar-white" />
                <span>CPU Load</span>
              </span>
            </div>
            <div className="flex items-baseline justify-between mt-1">
              <span className="text-lg font-bold text-lunar-white font-mono tracking-tight">
                {cpuVal.toFixed(0)}%
              </span>
              {(() => {
                const b = getMetricBadge(cpuVal, 75, 50);
                return (
                  <span className={`text-[9px] font-mono px-1.5 py-0.5 rounded ${b.className}`}>
                    {b.label}
                  </span>
                );
              })()}
            </div>
          </div>
        )}

        {/* RAM Pill */}
        {showRam && overview && (
          <div className="p-3 rounded-xl bg-lunar-surface border border-lunar-border hover:bg-lunar-surface-2 transition-all flex flex-col justify-between shadow-xs">
            <div className="flex items-center justify-between text-lunar-text-sec text-[10px] mb-1">
              <span className="flex items-center gap-1.5 font-medium text-lunar-text">
                <Activity className="w-3.5 h-3.5 text-lunar-white" />
                <span>Memory</span>
              </span>
              <span className="text-[10px] text-lunar-muted font-mono truncate max-w-[75px]">
                {overview.memoryUsed}
              </span>
            </div>
            <div className="flex items-baseline justify-between mt-1">
              <span className="text-lg font-bold text-lunar-white font-mono tracking-tight">
                {ramVal.toFixed(0)}%
              </span>
              {(() => {
                const b = getMetricBadge(ramVal, 85, 70);
                return (
                  <span className={`text-[9px] font-mono px-1.5 py-0.5 rounded ${b.className}`}>
                    {b.label}
                  </span>
                );
              })()}
            </div>
          </div>
        )}

        {/* Storage Pill */}
        {showDisk && (primaryDrive ? (
          <div className="p-3 rounded-xl bg-lunar-surface border border-lunar-border hover:bg-lunar-surface-2 transition-all flex flex-col justify-between shadow-xs">
            <div className="flex items-center justify-between text-lunar-text-sec text-[10px] mb-1">
              <span className="flex items-center gap-1.5 font-medium text-lunar-text">
                <HardDrive className="w-3.5 h-3.5 text-lunar-white" />
                <span>Disk {primaryDrive.drive}</span>
              </span>
              <span className="text-[10px] text-lunar-muted font-mono truncate max-w-[75px]">
                {primaryDrive.free} free
              </span>
            </div>
            <div className="flex items-baseline justify-between mt-1">
              <span className="text-lg font-bold text-lunar-white font-mono tracking-tight">
                {diskVal.toFixed(0)}%
              </span>
              {(() => {
                const b = getMetricBadge(diskVal, 90, 75);
                return (
                  <span className={`text-[9px] font-mono px-1.5 py-0.5 rounded ${b.className}`}>
                    {b.label}
                  </span>
                );
              })()}
            </div>
          </div>
        ) : overview ? (
          <div className="p-3 rounded-xl bg-lunar-surface border border-lunar-border hover:bg-lunar-surface-2 transition-all flex flex-col justify-between shadow-xs">
            <div className="flex items-center justify-between text-lunar-text-sec text-[10px] mb-1">
              <span className="flex items-center gap-1.5 font-medium text-lunar-text">
                <Activity className="w-3.5 h-3.5 text-lunar-white" />
                <span>Strain</span>
              </span>
            </div>
            <div className="flex items-baseline justify-between mt-1">
              <span className="text-lg font-bold text-lunar-white font-mono tracking-tight">
                {overview.systemStrainPercent}%
              </span>
              {(() => {
                const b = getMetricBadge(overview.systemStrainPercent, 70, 45);
                return (
                  <span className={`text-[9px] font-mono px-1.5 py-0.5 rounded ${b.className}`}>
                    {b.label}
                  </span>
                );
              })()}
            </div>
          </div>
        ) : null)}

        {/* Startups Pill */}
        {showStartup && startup && (
          <div className="p-3 rounded-xl bg-lunar-surface border border-lunar-border hover:bg-lunar-surface-2 transition-all flex flex-col justify-between shadow-xs">
            <div className="flex items-center justify-between text-lunar-text-sec text-[10px] mb-1">
              <span className="flex items-center gap-1.5 font-medium text-lunar-text">
                <Rocket className="w-3.5 h-3.5 text-lunar-white" />
                <span>Startups</span>
              </span>
              <span className="text-[10px] text-lunar-muted font-mono">
                {startup.disabledCount} off
              </span>
            </div>
            <div className="flex items-baseline justify-between mt-1">
              <span className="text-lg font-bold text-lunar-white font-mono tracking-tight">
                {startupCount}
              </span>
              {highImpactCount > 0 ? (
                <span className="text-[9px] font-mono font-medium px-1.5 py-0.5 rounded bg-lunar-white text-lunar-bg">
                  {highImpactCount} Heavy
                </span>
              ) : (
                <span className="text-[9px] font-mono font-medium px-1.5 py-0.5 rounded bg-lunar-surface-2 text-lunar-text-sec border border-lunar-border">
                  Optimal
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
