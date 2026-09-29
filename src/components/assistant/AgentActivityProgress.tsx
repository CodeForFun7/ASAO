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
        className="inline-flex items-center gap-2 px-3 py-1.5 rounded-full bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.08] text-zinc-300 transition-colors cursor-pointer select-none"
      >
        {isStreaming ? (
          <div className="flex items-center gap-1.5 text-white font-medium text-[11px]">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-white" />
            </span>
            <span>
              {currentRunningStep?.label || "Analyzing system telemetry..."}
            </span>
          </div>
        ) : hasError ? (
          <div className="flex items-center gap-1.5 text-zinc-300 text-[11px]">
            <Sparkles className="w-3 h-3 text-white" />
            <span>Analysis completed with notes</span>
          </div>
        ) : (
          <div className="flex items-center gap-1.5 text-zinc-300 text-[11px]">
            <Check className="w-3.5 h-3.5 text-white" />
            <span className="text-white font-medium">Diagnostics completed</span>
            <span className="text-zinc-500">({steps.length} checks)</span>
          </div>
        )}

        <div className="flex items-center pl-1 text-zinc-500">
          {isExpanded ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
        </div>
      </button>

      {/* Expanded Steps List */}
      {isExpanded && (
        <div className="mt-2 p-3 rounded-xl bg-white/[0.02] border border-white/[0.06] space-y-2 max-w-xl">
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
                      <Loader2 className="w-3 h-3 text-white animate-spin" />
                    ) : isDone ? (
                      <Check className="w-3 h-3 text-white" />
                    ) : isErr ? (
                      <span className="w-1.5 h-1.5 rounded-full bg-zinc-400 mt-1 inline-block" />
                    ) : (
                      <span className="w-1.5 h-1.5 rounded-full bg-zinc-600 mt-1 inline-block" />
                    )}
                  </div>

                  <div className="min-w-0">
                    <div className="text-zinc-200 font-medium truncate">
                      {step.label}
                    </div>
                    {step.detail && (
                      <div className="text-zinc-500 text-[10px] mt-0.5 truncate">
                        {step.detail}
                      </div>
                    )}
                  </div>
                </div>

                {step.durationMs !== undefined && (
                  <span className="font-mono text-zinc-500 text-[10px] shrink-0">
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
