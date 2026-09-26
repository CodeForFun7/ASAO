import React, { useState, useEffect } from "react";
import { X, Lock, Sparkles, FolderOpen } from "lucide-react";
import {
  CATEGORY_METADATA,
  type ProcessAnalysis,
  type ProcessInfo,
  type ProcessResourceSample,
} from "../../types/process";
import { formatBytes, formatRate, formatUptime } from "../../services/tauri";
import { processAnalysisService } from "../../services/process-analysis";
import { ProcessStatusBadge } from "./ProcessStatusBadge";
import { ProcessResourceChart } from "./ProcessResourceChart";

interface ProcessDetailsProps {
  process: ProcessInfo;
  samples: ProcessResourceSample[];
  onClose: () => void;
}

export const ProcessDetails: React.FC<ProcessDetailsProps> = ({
  process,
  samples,
  onClose,
}) => {
  const [aiAnalysis, setAiAnalysis] = useState<ProcessAnalysis | null>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);

  // Reset placeholder analysis when user selects a different PID
  useEffect(() => {
    setAiAnalysis(null);
    setIsAnalyzing(false);
  }, [process.pid]);

  const handleAnalyzeWithAi = async () => {
    setIsAnalyzing(true);
    try {
      const res = await processAnalysisService.analyzeProcess(process);
      setAiAnalysis(res);
    } finally {
      setIsAnalyzing(false);
    }
  };

  const categoryMeta =
    CATEGORY_METADATA[process.category] ?? CATEGORY_METADATA.unknown;
  const isActive =
    process.status === "active" ||
    process.cpuPercent >= 1.0 ||
    process.diskBytesPerSec > 0;

  return (
    <aside className="w-96 bg-[#101318] border-l border-lunar-border flex flex-col h-full shrink-0 overflow-hidden">
      {/* Top Panel Header */}
      <div className="p-4 border-b border-lunar-border flex items-start justify-between gap-3 bg-lunar-surface-2/50">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h2 className="text-sm font-semibold text-lunar-white truncate">
              {process.name}
            </h2>
            <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-lunar-bg text-lunar-muted border border-lunar-border shrink-0">
              PID {process.pid}
            </span>
          </div>
          <div className="mt-1 flex items-center gap-2">
            <span className="text-xs text-lunar-text-sec">
              {categoryMeta.label}
            </span>
            <span className="text-lunar-border">•</span>
            <ProcessStatusBadge status={process.status} compact />
          </div>
        </div>

        <button
          type="button"
          onClick={onClose}
          className="p-1 rounded hover:bg-lunar-elevated text-lunar-muted hover:text-lunar-white transition-colors cursor-pointer"
          title="Close process details"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Scrollable Body */}
      <div className="flex-1 overflow-y-auto p-4 space-y-5">
        {/* Protected / System Critical Notice */}
        {process.isSystemCritical && (
          <div className="p-3 rounded-md bg-lunar-ai/10 border border-lunar-ai/25 flex items-start gap-2.5">
            <Lock className="w-3.5 h-3.5 text-lunar-ai shrink-0 mt-0.5" />
            <div className="text-[11px] text-lunar-text-sec leading-relaxed">
              <strong className="text-lunar-ai font-medium block">
                Protected Windows System Process
              </strong>
              Essential operating system service. Modifying or terminating this
              process is locked to maintain system stability.
            </div>
          </div>
        )}

        {/* RESOURCE USAGE */}
        <section className="space-y-3">
          <h3 className="text-[10px] font-mono uppercase tracking-[0.15em] text-lunar-muted">
            Resource Usage
          </h3>

          {/* CPU Bar */}
          <div className="space-y-1">
            <div className="flex items-center justify-between text-xs font-mono">
              <span className="text-lunar-text-sec">CPU</span>
              <span className="text-lunar-white">
                {process.cpuPercent.toFixed(1)}%
              </span>
            </div>
            <div className="h-2 w-full bg-lunar-bg rounded-sm overflow-hidden border border-lunar-border p-[1px]">
              <div
                className={`h-full transition-all duration-300 ${
                  process.cpuPercent >= 25
                    ? "bg-lunar-warning"
                    : "bg-lunar-white"
                }`}
                style={{
                  width: `${Math.max(2, Math.min(100, process.cpuPercent))}%`,
                }}
              />
            </div>
          </div>

          {/* Memory Bar */}
          <div className="space-y-1">
            <div className="flex items-center justify-between text-xs font-mono">
              <span className="text-lunar-text-sec">Memory</span>
              <span className="text-lunar-white">
                {process.isRestricted && process.memoryBytes === 0
                  ? "Restricted"
                  : formatBytes(process.memoryBytes)}
              </span>
            </div>
            <div className="h-2 w-full bg-lunar-bg rounded-sm overflow-hidden border border-lunar-border p-[1px]">
              <div
                className={`h-full transition-all duration-300 ${
                  process.memoryBytes >= 800 * 1024 * 1024
                    ? "bg-lunar-warning"
                    : "bg-lunar-text"
                }`}
                style={{
                  width: `${Math.max(
                    2,
                    Math.min(
                      100,
                      (process.memoryBytes / (2 * 1024 * 1024 * 1024)) * 100
                    )
                  )}%`,
                }}
              />
            </div>
          </div>

          {/* Disk & Network Readouts */}
          <div className="grid grid-cols-2 gap-2 pt-1">
            <div className="p-2.5 rounded bg-lunar-bg border border-lunar-border flex items-center justify-between font-mono text-xs">
              <span className="text-lunar-muted">Disk</span>
              <span className="text-lunar-text">
                {formatRate(process.diskBytesPerSec)}
              </span>
            </div>
            <div className="p-2.5 rounded bg-lunar-bg border border-lunar-border flex items-center justify-between font-mono text-xs">
              <span className="text-lunar-muted">Network</span>
              <span className="text-lunar-text">
                {formatRate(process.networkBytesPerSec)}
              </span>
            </div>
          </div>
        </section>

        {/* RESOURCE HISTORY GRAPH */}
        <section className="space-y-2">
          <h3 className="text-[10px] font-mono uppercase tracking-[0.15em] text-lunar-muted">
            Telemetry History
          </h3>
          <ProcessResourceChart samples={samples} />
        </section>

        {/* ACTIVITY & METADATA */}
        <section className="space-y-2.5 border-t border-lunar-border pt-4">
          <h3 className="text-[10px] font-mono uppercase tracking-[0.15em] text-lunar-muted">
            Activity &amp; Metadata
          </h3>

          <div className="rounded-md bg-lunar-bg border border-lunar-border divide-y divide-lunar-border/60 text-xs font-mono">
            <div className="px-3 py-2 flex items-center justify-between">
              <span className="text-lunar-muted">Started</span>
              <span className="text-lunar-text">
                {formatUptime(process.startedSecondsAgo)}
              </span>
            </div>
            <div className="px-3 py-2 flex items-center justify-between">
              <span className="text-lunar-muted">Active</span>
              <span className="text-lunar-text">
                {isActive ? "Yes" : "No"}
              </span>
            </div>
            <div className="px-3 py-2 flex items-center justify-between">
              <span className="text-lunar-muted">Startup</span>
              <span className="text-lunar-text">
                {process.isStartup ? "Enabled" : "On-Demand"}
              </span>
            </div>
            <div className="px-3 py-2 flex items-center justify-between">
              <span className="text-lunar-muted">Threads</span>
              <span className="text-lunar-text">{process.threadCount}</span>
            </div>
            <div className="px-3 py-2 flex items-center justify-between">
              <span className="text-lunar-muted">Parent PID</span>
              <span className="text-lunar-text">
                {process.parentPid ?? "—"}
              </span>
            </div>
            <div className="px-3 py-2 flex items-center justify-between gap-2">
              <span className="text-lunar-muted shrink-0">Publisher</span>
              <span className="text-lunar-text truncate text-right">
                {process.publisher ?? (process.isRestricted ? "Restricted" : "Unknown")}
              </span>
            </div>
            <div className="px-3 py-2 flex items-center justify-between gap-2">
              <span className="text-lunar-muted shrink-0">Product</span>
              <span className="text-lunar-text truncate text-right">
                {process.productName ?? (process.isRestricted ? "Restricted" : "—")}
              </span>
            </div>
          </div>

          <div className="p-2.5 rounded bg-lunar-bg border border-lunar-border space-y-1">
            <div className="flex items-center gap-1.5 text-[10px] font-mono text-lunar-muted">
              <FolderOpen className="w-3 h-3" />
              <span>Executable Path</span>
            </div>
            <div className="text-[11px] font-mono text-lunar-text-sec break-all leading-relaxed">
              {process.executablePath ??
                (process.isRestricted
                  ? "Restricted (Kernel / Elevated Protection)"
                  : "Unavailable")}
            </div>
          </div>
        </section>

        {/* AI ANALYSIS PLACEHOLDER */}
        <section className="space-y-3 border-t border-lunar-border pt-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
              <h3 className="text-[10px] font-mono uppercase tracking-[0.15em] text-lunar-ai">
                AI Analysis
              </h3>
            </div>
            <span className="text-[10px] font-mono text-lunar-muted">
              PLACEHOLDER
            </span>
          </div>

          <div className="p-3 rounded-md bg-lunar-bg border border-lunar-border space-y-3">
            <p className="text-xs text-lunar-text-sec leading-relaxed">
              {aiAnalysis
                ? aiAnalysis.summary
                : "AI analysis will be available here once the LLM service is connected."}
            </p>

            {aiAnalysis && (
              <p className="text-[11px] font-mono text-lunar-muted border-t border-lunar-border pt-2 leading-relaxed">
                {aiAnalysis.reason}
              </p>
            )}

            <div className="grid grid-cols-2 gap-2 pt-1 border-t border-lunar-border/60 text-xs font-mono">
              <div className="flex items-center justify-between">
                <span className="text-lunar-muted">Risk</span>
                <span className="text-lunar-text-sec">
                  {aiAnalysis?.risk ?? "—"}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-lunar-muted">Confidence</span>
                <span className="text-lunar-text-sec">
                  {aiAnalysis?.confidence !== undefined
                    ? `${Math.round(aiAnalysis.confidence * 100)}%`
                    : "—"}
                </span>
              </div>
            </div>

            <button
              type="button"
              onClick={() => void handleAnalyzeWithAi()}
              disabled={isAnalyzing}
              className="w-full py-1.5 px-3 rounded bg-lunar-elevated hover:bg-lunar-border text-lunar-white border border-lunar-border text-xs font-medium transition-colors cursor-pointer disabled:opacity-50"
            >
              {isAnalyzing ? "Preparing Process Context..." : "Analyze with AI"}
            </button>
          </div>
        </section>
      </div>
    </aside>
  );
};
