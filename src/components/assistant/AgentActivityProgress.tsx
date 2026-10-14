import React, { useState } from "react";
import { Check, ChevronDown, ChevronUp, Loader2, Sparkles } from "lucide-react";
import type { DiagnosticStep } from "../../types/agent";

interface AgentActivityProgressProps {
  steps: DiagnosticStep[];
  isStreaming?: boolean;
}

export const AgentActivityProgress: React.FC<AgentActivityProgressProps> = ({
  steps,
  isStreaming = false,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  if (!steps || steps.length === 0) return null;

  const currentRunningStep = steps.find((s) => s.status === "running");
  const hasError = steps.some((s) => s.status === "error");

  return (
    <div className="mb-3 text-xs">
      {/* Sleek Minimal Activity Badge / Header */}
      <button
        type="button"
        onClick={() => setIsExpanded(!isExpanded)}
        className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-lunar-surface hover:bg-lunar-elevated border border-lunar-border text-lunar-text transition-colors cursor-pointer select-none shadow-xs"
      >
        {isStreaming ? (
          <div className="flex items-center gap-1.5 text-lunar-white font-medium text-[11px]">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-lunar-white opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-lunar-white" />
            </span>
            <span>
              {currentRunningStep?.label || "Analyzing system telemetry..."}
            </span>
          </div>
        ) : hasError ? (
          <div className="flex items-center gap-1.5 text-lunar-text-sec text-[11px]">
            <Sparkles className="w-3 h-3 text-lunar-white" />
            <span>Analysis completed with notes</span>
          </div>
        ) : (
          <div className="flex items-center gap-1.5 text-lunar-text-sec text-[11px]">
            <Check className="w-3.5 h-3.5 text-lunar-healthy" />
            <span className="text-lunar-white font-medium">Diagnostics completed</span>
            <span className="text-lunar-muted">({steps.length} checks)</span>
          </div>
        )}

        <div className="flex items-center pl-1 text-lunar-muted">
          {isExpanded ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
        </div>
      </button>

      {/* Expanded Steps List */}
      {isExpanded && (
        <div className="mt-2 p-3 rounded-xl bg-lunar-surface border border-lunar-border space-y-2 max-w-xl shadow-xs">
          {steps.map((step) => {
            const isRun = step.status === "running";
            const isDone = step.status === "completed";
            const isErr = step.status === "error";

            return (
              <div
                key={step.id}
                className="flex items-start justify-between gap-2 text-[11px]"
              >
                <div className="flex items-start gap-2 min-w-0">
                  <div className="mt-0.5 shrink-0">
                    {isRun ? (
                      <Loader2 className="w-3 h-3 text-lunar-white animate-spin" />
                    ) : isDone ? (
                      <Check className="w-3 h-3 text-lunar-healthy" />
                    ) : isErr ? (
                      <span className="w-1.5 h-1.5 rounded-full bg-lunar-warning mt-1 inline-block" />
                    ) : (
                      <span className="w-1.5 h-1.5 rounded-full bg-lunar-muted mt-1 inline-block" />
                    )}
                  </div>

                  <div className="min-w-0">
                    <div className="text-lunar-text font-medium truncate">
                      {step.label}
                    </div>
                    {step.detail && (
                      <div className="text-lunar-muted text-[10px] mt-0.5 truncate">
                        {step.detail}
                      </div>
                    )}
                  </div>
                </div>

                {step.durationMs !== undefined && (
                  <span className="font-mono text-lunar-muted text-[10px] shrink-0">
                    {step.durationMs}ms
                  </span>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
