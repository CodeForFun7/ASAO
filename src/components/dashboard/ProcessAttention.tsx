import React from "react";
import { AlertCircle, ArrowRight } from "lucide-react";
import type { ProcessInfo } from "../../types/process";
import { formatBytes } from "../../services/tauri";

interface ProcessAttentionProps {
  attentionProcesses: ProcessInfo[];
  onViewAttentionProcesses: () => void;
  onInspectProcess: (pid: number) => void;
}

export const ProcessAttention: React.FC<ProcessAttentionProps> = ({
  attentionProcesses,
  onViewAttentionProcesses,
  onInspectProcess,
}) => {
  const count = attentionProcesses.length;
  const topCandidates = attentionProcesses.slice(0, 3);

  return (
    <section className="rounded-xl lunar-glass-card p-5 flex flex-col justify-between">
      <div>
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <AlertCircle className="w-3.5 h-3.5 text-lunar-warning" />
            <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-text-sec">
              Process Attention
            </h2>
          </div>
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-lunar-warning/10 text-lunar-warning border border-lunar-warning/30">
            {count} FLAGGED
          </span>
        </div>

        <p className="text-xs text-lunar-text leading-relaxed mb-3">
          {count === 0
            ? "All running processes are operating within nominal CPU and memory envelopes."
            : `${count} ${
                count === 1 ? "process" : "processes"
              } may require investigation.`}
        </p>

        <ul className="space-y-1.5 text-xs text-lunar-text-sec mb-4">
          <li className="flex items-center gap-2">
            <span className="w-1 h-1 rounded-full bg-lunar-warning" />
            <span>High memory usage (&gt; 900 MB working set)</span>
          </li>
          <li className="flex items-center gap-2">
            <span className="w-1 h-1 rounded-full bg-lunar-warning" />
            <span>Background activity &amp; persistent CPU load</span>
          </li>
          <li className="flex items-center gap-2">
            <span className="w-1 h-1 rounded-full bg-lunar-warning" />
            <span>Unusual resource usage by unclassified executables</span>
          </li>
        </ul>

        {topCandidates.length > 0 && (
          <div className="space-y-1 border-t border-lunar-border pt-2.5">
            {topCandidates.map((proc) => (
              <div
                key={proc.pid}
                onClick={() => onInspectProcess(proc.pid)}
                className="flex items-center justify-between px-2 py-1 rounded bg-lunar-bg/70 hover:bg-lunar-elevated border border-lunar-border/60 text-[11px] font-mono cursor-pointer transition-colors"
              >
                <span className="text-lunar-text truncate max-w-[160px]">
                  {proc.name}
                </span>
                <span className="text-lunar-text-sec">
                  {proc.cpuPercent.toFixed(1)}% CPU ·{" "}
                  {formatBytes(proc.memoryBytes)}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="mt-5 pt-3 border-t border-lunar-border flex items-center justify-between">
        <button
          type="button"
          onClick={onViewAttentionProcesses}
          className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded bg-lunar-elevated hover:bg-lunar-border text-lunar-white border border-lunar-border text-xs font-medium transition-colors cursor-pointer"
        >
          <span>View Processes</span>
          <ArrowRight className="w-3.5 h-3.5" />
        </button>
        <span className="text-[10px] font-mono text-lunar-muted">
          FILTER: ATTENTION
        </span>
      </div>
    </section>
  );
};
