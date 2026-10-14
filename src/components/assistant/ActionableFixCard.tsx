import React from "react";
import { Wrench, Check, Loader2, ArrowUpRight, FolderOpen, PowerOff } from "lucide-react";
import type { ActionableFix } from "../../types/agent";

interface ActionableFixCardProps {
  fix: ActionableFix;
  onApply: (fixId: string) => void;
}

export const ActionableFixCard: React.FC<ActionableFixCardProps> = ({ fix, onApply }) => {
  const isApplying = fix.status === "applying";
  const isApplied = fix.status === "applied";
  const isFailed = fix.status === "failed";

  const getActionIcon = () => {
    switch (fix.actionType) {
      case "disable_startup":
        return <PowerOff className="w-3.5 h-3.5 text-lunar-white" />;
      case "inspect_process":
        return <ArrowUpRight className="w-3.5 h-3.5 text-lunar-white" />;
      case "open_storage":
        return <FolderOpen className="w-3.5 h-3.5 text-lunar-white" />;
      default:
        return <Wrench className="w-3.5 h-3.5 text-lunar-text-sec" />;
    }
  };

  const getButtonLabel = () => {
    if (isApplying) return "Applying...";
    if (isApplied) return fix.feedbackMessage || "✓ Applied";
    if (isFailed) return "Retry";

    switch (fix.actionType) {
      case "disable_startup":
        return "Disable Startup";
      case "inspect_process":
        return "Inspect Process";
      case "open_storage":
        return "Open Location";
      default:
        return "Apply Fix";
    }
  };

  return (
    <div className="flex items-center justify-between gap-3 p-3 rounded-xl border border-lunar-border bg-lunar-surface hover:bg-lunar-surface-2 transition-all text-xs shadow-xs">
      <div className="flex items-start gap-2.5 min-w-0">
        <div className="p-2 rounded-lg bg-lunar-surface-2 border border-lunar-border shrink-0 mt-0.5">
          {getActionIcon()}
        </div>

        <div className="min-w-0">
          <div className="font-medium text-lunar-white text-[12.5px] truncate">
            {fix.title}
          </div>
          <div className="text-[11px] text-lunar-text-sec mt-0.5 leading-tight">
            {fix.description}
          </div>
        </div>
      </div>

      <button
        type="button"
        disabled={isApplying || isApplied}
        onClick={() => onApply(fix.id)}
        className={`px-3 py-1.5 rounded-lg font-medium text-[11.5px] shrink-0 flex items-center gap-1.5 transition-all cursor-pointer ${
          isApplied
            ? "bg-lunar-elevated text-lunar-white border border-lunar-border cursor-default"
            : isFailed
            ? "bg-lunar-critical/15 text-lunar-critical border border-lunar-critical/30 hover:bg-lunar-critical/25"
            : isApplying
            ? "bg-lunar-surface-2 text-lunar-muted border border-lunar-border cursor-wait"
            : "bg-lunar-white text-lunar-bg hover:opacity-90 active:scale-95 shadow-xs"
        }`}
      >
        {isApplying && <Loader2 className="w-3 h-3 animate-spin text-lunar-bg" />}
        {isApplied && <Check className="w-3 h-3 text-lunar-white" />}
        <span>{getButtonLabel()}</span>
      </button>
    </div>
  );
};
