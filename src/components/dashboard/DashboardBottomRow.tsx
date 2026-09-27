import React, { useMemo, useState } from "react";
import { ArrowRight, Send, Sparkles, Zap } from "lucide-react";
import type { ProcessInfo, SystemMetrics } from "../../types/process";
import type { StructuredSystemContext } from "../../types/widget";
import { formatBytes } from "../../services/tauri";
import { chatService } from "../../services/chat";

interface DashboardBottomRowProps {
  metrics: SystemMetrics;
  processes: ProcessInfo[];
  onInspectProcess: (pid: number) => void;
  onOpenProcesses: () => void;
}

export const DashboardBottomRow: React.FC<DashboardBottomRowProps> = ({
  metrics,
  processes,
  onInspectProcess,
  onOpenProcesses,
}) => {
  const [promptInput, setPromptInput] = useState("");
  const [assistantReply, setAssistantReply] = useState<string | null>(null);
  const [isSending, setIsSending] = useState(false);

  // Derive deterministic recommendation from the Background Process Analyzer
  const recommendation = useMemo(() => {
    const bgCandidates = processes
      .filter(
        (p) => !p.isSystemCritical && p.activityState !== "foreground"
      )
      .sort(
        (a, b) =>
          (b.backgroundImpactScore ?? b.cpuPercent * 2) -
          (a.backgroundImpactScore ?? a.cpuPercent * 2)
      );

    const topCandidate = bgCandidates[0];
    const sustainedCpu = topCandidate
      ? topCandidate.sustainedCpuPercent ?? topCandidate.cpuPercent
      : 0;

    if (
      topCandidate &&
      (sustainedCpu >= 12 ||
        topCandidate.memoryBytes >= 380 * 1024 * 1024 ||
        topCandidate.status === "attention")
    ) {
      return {
        title: `Sustained Background Load: ${topCandidate.name}`,
        message: `${topCandidate.name} is consuming ${sustainedCpu.toFixed(
          1
        )}% CPU and ${formatBytes(
          topCandidate.memoryBytes
        )} RAM in the background while not in active focus.`,
        badge:
          topCandidate.impactLevel === "high"
            ? "HIGH IMPACT"
            : "MODERATE IMPACT",
        badgeTone:
          topCandidate.impactLevel === "high"
            ? "bg-lunar-critical/15 text-lunar-critical border-lunar-critical/35"
            : "bg-lunar-warning/15 text-lunar-warning border-lunar-warning/35",
        pid: topCandidate.pid,
      };
    }

    return {
      title: "Background Load Nominal",
      message: `All background processes are operating within nominal thresholds (${(
        metrics.backgroundLoadPercent ?? 0
      ).toFixed(0)}% background load · ${
        metrics.totalProcesses
      } active processes).`,
      badge: "NOMINAL",
      badgeTone:
        "bg-lunar-healthy/15 text-lunar-healthy border-lunar-healthy/30",
      pid: topCandidate?.pid ?? null,
    };
  }, [metrics, processes]);

  const handleAskAssistant = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const query =
      promptInput.trim() || "Which background process is adding the most load?";
    setIsSending(true);
    setPromptInput("");

    try {
      const topCpu = [...processes]
        .sort((a, b) => b.cpuPercent - a.cpuPercent)
        .slice(0, 5)
        .map((p) => ({
          pid: p.pid,
          name: p.name,
          category: p.category,
          cpuPercent: p.cpuPercent,
          sustainedCpuPercent: p.sustainedCpuPercent ?? p.cpuPercent,
          memoryBytes: p.memoryBytes,
          status: p.status,
          activityState: p.activityState ?? "background",
          sustainedLoadSeconds: p.sustainedLoadSeconds ?? 0,
          impactLevel: p.impactLevel ?? "low",
        }));

      const topMem = [...processes]
        .sort((a, b) => b.memoryBytes - a.memoryBytes)
        .slice(0, 5)
        .map((p) => ({
          pid: p.pid,
          name: p.name,
          category: p.category,
          cpuPercent: p.cpuPercent,
          sustainedCpuPercent: p.sustainedCpuPercent ?? p.cpuPercent,
          memoryBytes: p.memoryBytes,
          status: p.status,
          activityState: p.activityState ?? "background",
          sustainedLoadSeconds: p.sustainedLoadSeconds ?? 0,
          impactLevel: p.impactLevel ?? "low",
        }));

      const context: StructuredSystemContext = {
        cpuUsage: metrics.cpuUsagePercent,
        ramUsage: metrics.memoryUsagePercent,
        memoryUsedBytes: metrics.memoryUsedBytes,
        memoryTotalBytes: metrics.memoryTotalBytes,
        gpuUsage: metrics.gpuUsagePercent ?? 0,
        foregroundLoad: metrics.foregroundLoadPercent ?? 0,
        backgroundLoad: metrics.backgroundLoadPercent ?? 0,
        systemStrain:
          metrics.systemStrainPercent ??
          Math.round(
            metrics.cpuUsagePercent * 0.45 + metrics.memoryUsagePercent * 0.4
          ),
        userActive: metrics.userActive ?? true,
        foregroundProcessName: metrics.foregroundProcessName ?? null,
        processCount: metrics.totalProcesses,
        attentionCount: metrics.attentionProcesses,
        systemCondition:
          metrics.systemStatus === "critical"
            ? "ATTENTION"
            : metrics.systemStatus === "warning"
            ? "ELEVATED"
            : "GOOD",
        conditionReason: recommendation.title,
        topCpuProcesses: topCpu,
        topMemoryProcesses: topMem,
        recentResourceHistory: [],
        activeRecommendation: {
          id: "dash-rec",
          title: recommendation.title,
          message: recommendation.message,
          priority: "interesting",
          processPid: recommendation.pid,
          processName: null,
          metricHighlight: null,
        },
      };

      const res = await chatService.sendMessage(query, context);
      setAssistantReply(res.text);
    } finally {
      setIsSending(false);
    }
  };

  return (
    <div className="grid grid-cols-1 lg:grid-cols-12 gap-4 items-stretch">
      {/* Left Box (8 Cols): Recommendation Here */}
      <section className="lg:col-span-8 rounded-2xl lunar-glass-card p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div className="space-y-1.5 min-w-0 flex-1">
          <div className="flex items-center gap-2.5 flex-wrap">
            <span className="inline-flex items-center gap-1.5 text-xs font-semibold text-lunar-white tracking-tight">
              <Zap className="w-3.5 h-3.5 text-lunar-warning shrink-0" />
              <span>Recommendation</span>
            </span>
            <span
              className={`text-[10px] font-mono px-2 py-0.5 rounded-full border ${recommendation.badgeTone}`}
            >
              {recommendation.badge}
            </span>
          </div>

          <h3 className="text-sm font-semibold text-lunar-white truncate">
            {recommendation.title}
          </h3>
          <p className="text-xs text-lunar-text-sec leading-relaxed">
            {recommendation.message}
          </p>
        </div>

        <div className="shrink-0">
          <button
            type="button"
            onClick={() => {
              if (recommendation.pid) {
                onInspectProcess(recommendation.pid);
              } else {
                onOpenProcesses();
              }
            }}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-xl bg-lunar-elevated hover:bg-lunar-border text-lunar-white border border-lunar-border text-xs font-medium transition-colors cursor-pointer"
          >
            <span>
              {recommendation.pid ? "Inspect Process" : "View Processes"}
            </span>
            <ArrowRight className="w-3.5 h-3.5" />
          </button>
        </div>
      </section>

      {/* Right Box (4 Cols): AI Assistant Here */}
      <section className="lg:col-span-4 rounded-2xl lunar-glass-card p-5 flex flex-col justify-between gap-3">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
            <span className="text-sm font-semibold text-lunar-white tracking-tight">
              AI Assistant
            </span>
          </div>
          <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-lunar-ai/10 text-lunar-ai border border-lunar-ai/25">
            ASAO AI
          </span>
        </div>

        <div className="text-xs text-lunar-text-sec leading-relaxed line-clamp-3">
          {assistantReply
            ? assistantReply
            : "Ask Asao AI why your system feels heavy or which background processes can be safely closed."}
        </div>

        <form
          onSubmit={(e) => void handleAskAssistant(e)}
          className="flex items-center gap-2 pt-1"
        >
          <input
            type="text"
            value={promptInput}
            onChange={(e) => setPromptInput(e.target.value)}
            placeholder="Ask Asao AI..."
            className="flex-1 h-8 px-3 rounded-lg bg-lunar-bg/90 border border-lunar-border text-xs text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec transition-colors"
          />
          <button
            type="submit"
            disabled={isSending}
            className="h-8 px-3 rounded-lg bg-lunar-elevated hover:bg-lunar-border border border-lunar-border text-lunar-white text-xs font-medium inline-flex items-center gap-1.5 transition-colors cursor-pointer disabled:opacity-50"
            title="Ask Asao AI"
          >
            <Send className="w-3 h-3" />
          </button>
        </form>
      </section>
    </div>
  );
};
