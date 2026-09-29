import React from "react";
import { Cpu, Activity, Laptop, Layers } from "lucide-react";
import type { DiagnosticVisualData } from "../../types/agent";

interface ResourceUsageChartProps {
  data: DiagnosticVisualData;
}

export const ResourceUsageChart: React.FC<ResourceUsageChartProps> = ({ data }) => {
  const overview = data.overview;
  if (!overview) return null;

  const cpuPercent = Math.min(100, Math.max(0, overview.cpuUsagePercent));
  const ramPercent = Math.min(100, Math.max(0, overview.memoryUsagePercent));
  const bgLoad = overview.backgroundLoadPercent ?? 0;
  const fgLoad = overview.foregroundLoadPercent ?? 0;
  const fgName = overview.foregroundProcessName || "Active Window";

  return (
    <div className="rounded-2xl bg-white/[0.03] border border-white/[0.08] p-4 my-3 text-xs">
      <div className="flex items-center justify-between mb-3.5">
        <div className="flex items-center gap-2 font-medium text-white text-[12.5px]">
          <Activity className="w-4 h-4 text-white" />
          <span>Resource Usage Breakdown</span>
        </div>
        <div className="text-[11px] text-zinc-400 flex items-center gap-1.5">
          <Laptop className="w-3.5 h-3.5 text-zinc-400" />
          <span className="truncate max-w-[150px]">{fgName}</span>
        </div>
      </div>

      <div className="space-y-3.5 text-xs">
        {/* CPU Bar */}
        <div>
          <div className="flex items-center justify-between text-[11px] mb-1.5">
            <span className="flex items-center gap-1.5 text-zinc-300 font-medium">
              <Cpu className="w-3.5 h-3.5 text-zinc-400" />
              <span>CPU Utilization</span>
            </span>
            <span className="font-mono text-white font-semibold">
              {cpuPercent.toFixed(1)}%
            </span>
          </div>

          <div className="h-2 w-full rounded-full bg-white/[0.08] overflow-hidden flex">
            {/* Foreground load share: pure white */}
            <div
              style={{ width: `${Math.min(100, fgLoad)}%` }}
              className="h-full bg-white transition-all duration-500"
              title={`Foreground: ${fgLoad.toFixed(1)}%`}
            />
            {/* Background load share: zinc-400 */}
            <div
              style={{ width: `${Math.min(100, bgLoad)}%` }}
              className="h-full bg-zinc-400 transition-all duration-500"
              title={`Background: ${bgLoad.toFixed(1)}%`}
            />
          </div>

          <div className="flex items-center justify-between text-[10px] text-zinc-400 mt-1.5">
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-white inline-block" />
              <span>Foreground: {fgLoad.toFixed(0)}%</span>
            </span>
            <span className="flex items-center gap-1.5">
              <span className="w-1.5 h-1.5 rounded-full bg-zinc-400 inline-block" />
              <span>Background: {bgLoad.toFixed(0)}%</span>
            </span>
            <span>Idle: {Math.max(0, 100 - cpuPercent).toFixed(0)}%</span>
          </div>
        </div>

        {/* RAM Bar */}
        <div>
          <div className="flex items-center justify-between text-[11px] mb-1.5">
            <span className="flex items-center gap-1.5 text-zinc-300 font-medium">
              <Layers className="w-3.5 h-3.5 text-zinc-400" />
              <span>RAM Allocation</span>
            </span>
            <span className="font-mono text-white font-semibold">
              {ramPercent.toFixed(1)}% ({overview.memoryUsed} / {overview.memoryTotal})
            </span>
          </div>

          <div className="h-2 w-full rounded-full bg-white/[0.08] overflow-hidden">
            <div
              style={{ width: `${ramPercent}%` }}
              className="h-full bg-white transition-all duration-500"
            />
          </div>
        </div>
      </div>
    </div>
  );
};
