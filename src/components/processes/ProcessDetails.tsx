import React, { useState, useEffect } from "react";
import { X, Sparkles, FolderOpen } from "lucide-react";
import {
  CATEGORY_METADATA,
  type ProcessAnalysis,
  type ProcessInfo,
  type ProcessResourceSample,
} from "../../types/process";
import { formatBytes, formatRate, formatUptime } from "../../services/tauri";
import { processAnalysisService } from "../../services/process-analysis";
import { ProcessResourceChart } from "./ProcessResourceChart";

interface ProcessDetailsProps {
  process: ProcessInfo;
  samples: ProcessResourceSample[];
  onClose: () => void;
}

function getResourceTag(process: ProcessInfo) {
  if (
    process.cpuPercent >= 12 ||
    process.memoryBytes >= 1500 * 1024 * 1024 ||
    process.status === "high-resource" ||
    process.status === "attention"
  ) {
    return {
      label: "High",
      dot: "bg-lunar-white",
      badge: "bg-lunar-white/10 text-lunar-white border-lunar-white/25",
    };
  }
  if (process.cpuPercent >= 5 || process.memoryBytes >= 500 * 1024 * 1024) {
    return {
      label: "Moderate",
      dot: "bg-lunar-white",
      badge: "bg-lunar-white/10 text-lunar-white border-lunar-white/25",
    };
  }
  return {
    label: "Low",
    dot: "bg-lunar-white",
    badge: "bg-lunar-white/10 text-lunar-white border-lunar-white/25",
  };
}

function getActivityTag(process: ProcessInfo, isActive: boolean) {
  const state =
    process.activityState ?? (isActive ? "foreground" : "background");
  if (state === "foreground") {
    return {
      label: "In Use (Foreground)",
      shortLabel: "Foreground",
      dot: "bg-lunar-white",
      badge: "bg-lunar-white/10 text-lunar-white border-lunar-white/25",
    };
  }
  if (state === "background") {
    return {
      label: "Background",
      shortLabel: "Background",
      dot: "bg-lunar-white",
      badge: "bg-lunar-white/10 text-lunar-white border-lunar-white/25",
    };
  }
  return {
    label: "Idle",
    shortLabel: "Idle",
    dot: "bg-lunar-white",
    badge: "bg-lunar-white/10 text-lunar-white border-lunar-white/25",
  };
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

  const resourceTag = getResourceTag(process);
  const activityTag = getActivityTag(process, isActive);

  return (
    <aside className="w-96 bg-lunar-surface border-l border-lunar-border flex flex-col h-full shrink-0 overflow-hidden">
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
          <div className="mt-1 text-xs text-lunar-text-sec">
            {categoryMeta.label}
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            <span
              className={`inline-flex items-center gap-1.5 rounded border px-2 py-0.5 text-[11px] font-mono ${resourceTag.badge}`}
              title="Resource Level: How much is it consuming?"
            >
              <span
                className={`w-1.5 h-1.5 rounded-full shrink-0 ${resourceTag.dot}`}
              />
              <span>{resourceTag.label} Resource</span>
            </span>
            <span
              className={`inline-flex items-center gap-1.5 rounded border px-2 py-0.5 text-[11px] font-mono ${activityTag.badge}`}
              title="Activity: Is the user currently using it?"
            >
              <span
                className={`w-1.5 h-1.5 rounded-full shrink-0 ${activityTag.dot}`}
              />
              <span>{activityTag.shortLabel}</span>
            </span>
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
                className="h-full bg-lunar-white transition-all duration-300"
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
                className="h-full bg-lunar-white transition-all duration-300"
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
              <span className="text-lunar-muted">Activity State</span>
              <span className="text-lunar-text capitalize">
                {process.activityState ?? (isActive ? "Foreground" : "Background")}
              </span>
            </div>
            <div className="px-3 py-2 flex items-center justify-between">
              <span className="text-lunar-muted">Sustained Load</span>
              <span className="text-lunar-text">
                {process.sustainedLoadSeconds && process.sustainedLoadSeconds > 0
                  ? `${process.sustainedLoadSeconds}s`
                  : "Nominal"}
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

        {/* BACKGROUND LOAD & IMPACT ANALYSIS */}
        <section className="space-y-3 border-t border-lunar-border pt-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
              <h3 className="text-[10px] font-mono uppercase tracking-[0.15em] text-lunar-ai">
                Load &amp; Impact Analysis
              </h3>
            </div>
            <span className="text-[10px] font-mono text-lunar-muted uppercase">
              {process.impactLevel ?? "low"} impact
            </span>
          </div>

          <div className="p-3 rounded-md bg-lunar-bg border border-lunar-border space-y-3">
            <p className="text-xs text-lunar-text-sec leading-relaxed">
              {aiAnalysis
                ? aiAnalysis.summary
                : "Evaluate whether this process is unnecessarily adding background load while not in active focus."}
            </p>

            {aiAnalysis && (
              <div className="space-y-1.5 border-t border-lunar-border pt-2 text-[11px] font-mono">
                <p className="text-lunar-muted leading-relaxed">
                  {aiAnalysis.reason}
                </p>
                {aiAnalysis.recommendation && (
                  <p className="text-lunar-text-sec leading-relaxed">
                    {aiAnalysis.recommendation}
                  </p>
                )}
              </div>
            )}

            <div className="grid grid-cols-2 gap-2 pt-1 border-t border-lunar-border/60 text-xs font-mono">
              <div className="flex items-center justify-between">
                <span className="text-lunar-muted">Load Impact</span>
                <span className="text-lunar-text-sec capitalize">
                  {aiAnalysis?.impact ?? process.impactLevel ?? "low"}
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
              {isAnalyzing ? "Analyzing Background Impact..." : "Analyze Load Impact"}
            </button>
          </div>
        </section>
      </div>
    </aside>
  );
};
