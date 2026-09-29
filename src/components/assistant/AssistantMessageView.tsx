import React, { useState } from "react";
import { AlertCircle, Check, Copy, CheckCircle2 } from "lucide-react";
import type { ActionableFix, ChatMessage } from "../../types/agent";
import { AgentActivityProgress } from "./AgentActivityProgress";
import { MetricPillGroup } from "./MetricPillGroup";
import { ResourceUsageChart } from "./ResourceUsageChart";
import { ProcessBarChart } from "./ProcessBarChart";
import { StartupImpactChart } from "./StartupImpactChart";
import { StorageDistributionCard } from "./StorageDistributionCard";
import { ActionableFixCard } from "./ActionableFixCard";

interface AssistantMessageViewProps {
  message: ChatMessage;
  onApplyFix: (fixId: string, messageId: string) => void;
}

export const AssistantMessageView: React.FC<AssistantMessageViewProps> = ({
  message,
  onApplyFix,
}) => {
  const diagnostic = message.diagnosticData;
  const isStreaming = message.isStreaming;
  const hasSteps = Boolean(message.steps && message.steps.length > 0);
  const visibleWidgets = diagnostic?.visibleWidgets;
  const [copied, setCopied] = useState(false);

  // Determine which visual widgets to render based on model selection
  const showPills =
    (!visibleWidgets || visibleWidgets.includes("pills")) &&
    Boolean(diagnostic);

  const showStartupChart =
    (!visibleWidgets || visibleWidgets.includes("startup_chart")) &&
    Boolean(diagnostic?.startup && diagnostic.startup.highImpactItems.length > 0);

  const showResourceChart =
    (!visibleWidgets || visibleWidgets.includes("resource_chart")) &&
    Boolean(diagnostic?.overview);

  const showProcessChart =
    (!visibleWidgets || visibleWidgets.includes("process_chart")) &&
    Boolean(diagnostic?.processes && diagnostic.processes.topProcesses.length > 0);

  const showStorageChart =
    (!visibleWidgets || visibleWidgets.includes("storage_card")) &&
    Boolean(diagnostic?.storage);

  const showFixes =
    (!visibleWidgets || visibleWidgets.includes("actionable_fixes")) &&
    Boolean(message.fixes && message.fixes.length > 0);

  const handleCopy = () => {
    if (!message.text) return;
    void navigator.clipboard.writeText(message.text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="w-full text-zinc-200">
      {/* Subtle Step Execution Badge */}
      {hasSteps && (
        <AgentActivityProgress
          steps={message.steps || []}
          isStreaming={isStreaming}
        />
      )}

      {/* Error Notice */}
      {message.error && (
        <div className="mb-3 p-3 rounded-xl bg-white/[0.04] border border-white/10 flex items-center gap-2 text-xs text-white">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{message.error}</span>
        </div>
      )}

      {/* Natural Conversational Markdown Output */}
      {message.text && (
        <div className="text-[13.5px] leading-relaxed text-zinc-100 font-normal">
          {renderFormattedText(message.text)}
        </div>
      )}

      {/* Dynamic Visualizations Selected by the AI Model (Strictly White & Shades of White) */}
      {diagnostic && (
        <div className="mt-4 space-y-3">
          {/* 1. Selective Performance Metric Pills */}
          {showPills && <MetricPillGroup data={diagnostic} />}

          {/* 2. Startup Applications Impact Bar Chart */}
          {showStartupChart && <StartupImpactChart data={diagnostic} />}

          {/* 3. Resource Usage Breakdown Chart */}
          {showResourceChart && <ResourceUsageChart data={diagnostic} />}

          {/* 4. Top Background Processes Bar Chart */}
          {showProcessChart && <ProcessBarChart data={diagnostic} />}

          {/* 5. Storage Distribution & Capacity Card */}
          {showStorageChart && <StorageDistributionCard data={diagnostic} />}
        </div>
      )}

      {/* 6. Available Fix Action Cards */}
      {showFixes && message.fixes && message.fixes.length > 0 && (
        <div className="mt-4 space-y-2">
          <div className="text-[11px] font-medium uppercase tracking-wider text-zinc-400 flex items-center gap-1.5">
            <CheckCircle2 className="w-3.5 h-3.5 text-white" />
            <span>Available Actions ({message.fixes.length})</span>
          </div>
          <div className="space-y-2">
            {message.fixes.map((fix: ActionableFix) => (
              <ActionableFixCard
                key={fix.id}
                fix={fix}
                onApply={(fixId) => onApplyFix(fixId, message.id)}
              />
            ))}
          </div>
        </div>
      )}

      {/* Message Footer Actions (Copy response, etc.) */}
      {message.text && !isStreaming && (
        <div className="mt-3 flex items-center gap-2 text-zinc-500">
          <button
            type="button"
            onClick={handleCopy}
            className="p-1 rounded-md hover:bg-white/[0.06] hover:text-zinc-300 transition-colors cursor-pointer text-[11px] flex items-center gap-1"
            title="Copy response to clipboard"
          >
            {copied ? (
              <>
                <Check className="w-3.5 h-3.5 text-white" />
                <span className="text-white">Copied</span>
              </>
            ) : (
              <>
                <Copy className="w-3.5 h-3.5" />
                <span>Copy</span>
              </>
            )}
          </button>
        </div>
      )}
    </div>
  );
};

function renderFormattedText(text: string) {
  const lines = text.split("\n");
  return (
    <div className="space-y-2.5">
      {lines.map((line, idx) => {
        const trimmed = line.trim();
        if (!trimmed) return <div key={idx} className="h-1" />;

        // Header level 2 / 3
        if (trimmed.startsWith("## ") || trimmed.startsWith("### ")) {
          const headerText = trimmed.replace(/^#{2,3}\s+/, "");
          return (
            <h3
              key={idx}
              className="text-[14px] font-semibold text-white tracking-tight mt-4 mb-2"
            >
              {headerText}
            </h3>
          );
        }

        // Numbered list
        const numberMatch = trimmed.match(/^(\d+)\.\s+(.*)$/);
        if (numberMatch) {
          const num = numberMatch[1];
          const rest = numberMatch[2];
          return (
            <div key={idx} className="flex items-start gap-2.5 pl-1 my-1.5">
              <span className="w-5 h-5 rounded-full bg-white/[0.08] text-white text-[11px] font-mono font-medium flex items-center justify-center shrink-0 mt-0.5">
                {num}
              </span>
              <span className="flex-1 min-w-0 text-[13px] leading-relaxed text-zinc-200">
                {renderInlineFormatting(rest)}
              </span>
            </div>
          );
        }

        // Bullet item
        if (trimmed.startsWith("* ") || trimmed.startsWith("- ")) {
          return (
            <div key={idx} className="flex items-start gap-2.5 pl-2 my-1">
              <span className="w-1.5 h-1.5 rounded-full bg-zinc-400 mt-2 shrink-0 inline-block" />
              <span className="flex-1 min-w-0 text-[13px] leading-relaxed text-zinc-200">
                {renderInlineFormatting(trimmed.replace(/^[\*\-]\s+/, ""))}
              </span>
            </div>
          );
        }

        // Regular paragraph
        return (
          <p key={idx} className="my-1.5 text-[13px] leading-relaxed text-zinc-200">
            {renderInlineFormatting(line)}
          </p>
        );
      })}
    </div>
  );
}

function renderInlineFormatting(text: string) {
  const parts = text.split(/(`[^`]+`|\*\*[^*]+\*\*)/g);
  return parts.map((part, index) => {
    if (part.startsWith("`") && part.endsWith("`")) {
      return (
        <code
          key={index}
          className="px-1.5 py-0.5 rounded bg-white/[0.07] border border-white/10 font-mono text-[12px] text-white"
        >
          {part.slice(1, -1)}
        </code>
      );
    }
    if (part.startsWith("**") && part.endsWith("**")) {
      return (
        <strong key={index} className="font-semibold text-white">
          {part.slice(2, -2)}
        </strong>
      );
    }
    return part;
  });
}
