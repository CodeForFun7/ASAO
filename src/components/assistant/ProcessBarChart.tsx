import React from "react";
import { Cpu, ArrowUpRight, ShieldAlert } from "lucide-react";
import type { DiagnosticVisualData } from "../../types/agent";
import { useProcessStore } from "../../stores/process-store";

interface ProcessBarChartProps {
  data: DiagnosticVisualData;
}

export const ProcessBarChart: React.FC<ProcessBarChartProps> = ({ data }) => {
  const processes = data.processes?.topProcesses;
  const selectProcess = useProcessStore((s) => s.selectProcess);
  const setRoute = useProcessStore((s) => s.setRoute);

  if (!processes || processes.length === 0) return null;

  const topItems = processes.slice(0, 5);
  const maxCpu = Math.max(10, ...topItems.map((p) => p.sustainedCpuPercent || p.cpuPercent || 1));

  const handleInspect = (pid: number) => {
    selectProcess(pid);
    setRoute("processes");
  };

  return (
    <div className="rounded-2xl bg-lunar-surface border border-lunar-border p-4 my-3 text-xs shadow-xs">
      <div className="flex items-center justify-between mb-3.5">
        <div className="flex items-center gap-2 font-medium text-lunar-white text-[12.5px]">
          <Cpu className="w-4 h-4 text-lunar-white" />
          <span>Top Background Processes</span>
        </div>
        <span className="text-[11px] text-lunar-text-sec">
          Ranked by CPU &amp; Memory
        </span>
      </div>

      <div className="space-y-2.5">
        {topItems.map((p) => {
          const cpuVal = p.sustainedCpuPercent || p.cpuPercent;
          const barWidthPercent = Math.min(100, Math.max(8, (cpuVal / maxCpu) * 100));
          const isHeavy = cpuVal >= 10;

          return (
            <div
              key={p.pid}
              onClick={() => handleInspect(p.pid)}
              className="group p-2.5 rounded-xl bg-lunar-surface-2/60 border border-lunar-border hover:border-lunar-text-sec/40 hover:bg-lunar-surface-2 cursor-pointer transition-all"
            >
              <div className="flex items-center justify-between text-xs mb-1.5">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="font-semibold text-lunar-white truncate text-[12.5px] group-hover:text-lunar-text transition-colors">
                    {p.name}
                  </span>
                  <span className="font-mono text-[10px] text-lunar-muted">
                    PID {p.pid}
                  </span>
                  {p.isSystemCritical && (
                    <span className="text-[9px] px-1.5 py-0.5 rounded bg-lunar-elevated text-lunar-text-sec border border-lunar-border">
                      System
                    </span>
                  )}
                  {p.status === "attention" && (
                    <span className="text-[9px] px-1.5 py-0.5 rounded bg-lunar-white text-lunar-bg font-semibold flex items-center gap-1">
                      <ShieldAlert className="w-2.5 h-2.5" />
                      Attention
                    </span>
                  )}
                </div>

                <div className="flex items-center gap-3 shrink-0 text-right">
                  <span className="font-mono font-bold text-[12px] text-lunar-white">
                    {cpuVal.toFixed(1)}% CPU
                  </span>
                  <span className="font-mono text-[11px] text-lunar-text-sec">
                    {p.memoryFormatted}
                  </span>
                  <ArrowUpRight className="w-3 h-3 text-lunar-muted group-hover:text-lunar-white transition-colors opacity-0 group-hover:opacity-100" />
                </div>
              </div>

              {/* Horizontal Bar */}
              <div className="h-1.5 w-full rounded-full bg-lunar-border overflow-hidden">
                <div
                  style={{ width: `${barWidthPercent}%` }}
                  className={`h-full rounded-full transition-all duration-500 ${
                    isHeavy ? "bg-lunar-white" : "bg-lunar-muted"
                  }`}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
