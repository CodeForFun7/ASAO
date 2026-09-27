import React, { useState } from "react";
import { Sparkles, Terminal } from "lucide-react";
import type {
  ProcessAnalysis,
  ProcessInfo,
  SystemMetrics,
} from "../../types/process";
import { processAnalysisService } from "../../services/process-analysis";

interface AIInsightProps {
  metrics: SystemMetrics;
  attentionProcesses: ProcessInfo[];
}

export const AIInsight: React.FC<AIInsightProps> = ({
  metrics,
  attentionProcesses,
}) => {
  const [analysis, setAnalysis] = useState<ProcessAnalysis | null>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);

  const handleAnalyzeSystem = async () => {
    setIsAnalyzing(true);
    try {
      const res = await processAnalysisService.analyzeSystem(
        metrics,
        attentionProcesses
      );
      setAnalysis(res);
    } finally {
      setIsAnalyzing(false);
    }
  };

  const bgLoad = metrics.backgroundLoadPercent ?? 0;
  const fgLoad = metrics.foregroundLoadPercent ?? 0;

  return (
    <section className="rounded-xl lunar-glass-card p-5 flex flex-col justify-between">
      <div>
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
            <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-ai">
              Background Load &amp; Recommendations
            </h2>
          </div>
          <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-lunar-ai/10 text-lunar-ai border border-lunar-ai/25">
            {analysis ? `${(analysis.impact ?? "low").toUpperCase()} IMPACT` : "LOCAL ENGINE ACTIVE"}
          </span>
        </div>

        <p className="text-xs text-lunar-text-sec leading-relaxed">
          {analysis
            ? analysis.summary
            : `Evaluating sustained background load (${bgLoad.toFixed(
                0
              )}% background vs ${fgLoad.toFixed(
                0
              )}% foreground) while you use ${
                metrics.foregroundProcessName ?? "your PC"
              }.`}
        </p>

        {analysis ? (
          <div className="mt-3 p-3 rounded-lg lunar-glass-sub space-y-1.5 font-mono text-[11px]">
            <div className="text-lunar-text flex items-center gap-1.5">
              <Terminal className="w-3 h-3 text-lunar-ai shrink-0" />
              <span>Recommendation Engine Context</span>
            </div>
            <p className="text-lunar-text-sec leading-relaxed">
              {analysis.reason}
            </p>
            {analysis.recommendation && (
              <p className="text-lunar-muted pt-1 border-t border-lunar-border">
                {analysis.recommendation}
              </p>
            )}
          </div>
        ) : (
          <div className="mt-4 p-3 rounded-lg lunar-glass-sub text-[11px] font-mono text-lunar-muted space-y-1">
            <div>
              • Foreground Load: {fgLoad.toFixed(0)}% ({metrics.foregroundProcessName ?? "Desktop"})
            </div>
            <div>• Sustained Background Load: {bgLoad.toFixed(0)}%</div>
            <div>• Recommendation Engine: ACTIVE (Deterministic + LLM Ready)</div>
          </div>
        )}
      </div>

      <div className="mt-5 pt-3 border-t border-lunar-border flex items-center justify-between">
        <button
          type="button"
          onClick={() => void handleAnalyzeSystem()}
          disabled={isAnalyzing}
          className="px-3.5 py-1.5 rounded bg-lunar-elevated hover:bg-lunar-border text-lunar-white border border-lunar-border text-xs font-medium transition-colors cursor-pointer disabled:opacity-50"
        >
          {isAnalyzing ? "Evaluating Load..." : "Evaluate Background Load"}
        </button>
        <span className="text-[10px] font-mono text-lunar-muted">
          RecommendationEngine
        </span>
      </div>
    </section>
  );
};
