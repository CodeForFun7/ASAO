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
        return <PowerOff className="w-3.5 h-3.5 text-white" />;
      case "inspect_process":
        return <ArrowUpRight className="w-3.5 h-3.5 text-white" />;
      case "open_storage":
        return <FolderOpen className="w-3.5 h-3.5 text-white" />;
      default:
        return <Wrench className="w-3.5 h-3.5 text-zinc-300" />;
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
    <div className="flex items-center justify-between gap-3 p-3 rounded-xl border border-white/[0.08] bg-white/[0.02] hover:bg-white/[0.04] transition-all text-xs">
      <div className="flex items-start gap-2.5 min-w-0">
        <div className="p-2 rounded-lg bg-white/[0.05] border border-white/[0.08] shrink-0 mt-0.5">
          {getActionIcon()}
        </div>

        <div className="min-w-0">
          <div className="font-medium text-white text-[12.5px] truncate">
            {fix.title}
          </div>
          <div className="text-[11px] text-zinc-400 mt-0.5 leading-tight">
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
            ? "bg-white/10 text-white border border-white/20 cursor-default"
            : isFailed
            ? "bg-white/15 text-white border border-white/30 hover:bg-white/25"
            : isApplying
            ? "bg-white/[0.05] text-zinc-400 border border-white/10 cursor-wait"
            : "bg-white text-black hover:bg-zinc-200 active:scale-95 shadow-sm"
        }`}
      >
        {isApplying && <Loader2 className="w-3 h-3 animate-spin text-black" />}
        {isApplied && <Check className="w-3 h-3 text-white" />}
        <span>{getButtonLabel()}</span>
      </button>
    </div>
  );
};
