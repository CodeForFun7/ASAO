import React from "react";
import { ArrowUpRight, Sparkles } from "lucide-react";
import { useProcessStore } from "../../stores/process-store";

export const NyraCtaCard: React.FC = () => {
  const setRoute = useProcessStore((s) => s.setRoute);

  return (
    <button
      type="button"
      onClick={() => setRoute("assistant")}
      className="group h-full w-full min-h-[72px] rounded-xl lunar-glass-card-interactive px-4 py-3 flex items-center justify-between gap-3 text-left cursor-pointer"
    >
      <div className="flex items-center gap-3 min-w-0">
        <span className="w-9 h-9 rounded-lg bg-lunar-elevated border border-lunar-border flex items-center justify-center shrink-0">
          <Sparkles className="w-4 h-4 text-lunar-ai" />
        </span>
        <span className="min-w-0">
          <span className="block text-sm font-semibold text-lunar-white truncate">
            Chat with Nyra
          </span>
          <span className="block text-[11px] text-lunar-text-sec truncate">
            Ask about your PC, or let Nyra fix it
          </span>
        </span>
      </div>
      <ArrowUpRight className="w-4 h-4 text-lunar-muted group-hover:text-lunar-white transition-colors shrink-0" />
    </button>
  );
};
