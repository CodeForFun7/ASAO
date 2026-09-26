import React from "react";
import { Sparkles, ExternalLink } from "lucide-react";
import type { WidgetRecommendation } from "../../types/widget";
import { openMainWindow } from "../../services/widget";

interface RecommendationCardProps {
  recommendation: WidgetRecommendation;
  showRecommendations: boolean;
  onAskAsao: (promptQuestion?: string) => void;
}

export const RecommendationCard: React.FC<RecommendationCardProps> = ({
  recommendation,
  showRecommendations,
  onAskAsao,
}) => {
  const handleViewProcess = () => {
    void openMainWindow("processes", recommendation.processPid ?? undefined);
  };

  const handleAskAboutRecommendation = () => {
    if (recommendation.processName) {
      onAskAsao(
        `Why is ${recommendation.processName} using ${
          recommendation.metricHighlight ?? "high resources"
        }?`
      );
    } else {
      onAskAsao("How is my system performing right now?");
    }
  };

  const priorityBadge =
    recommendation.priority === "important"
      ? "text-lunar-critical border-lunar-critical/30 bg-lunar-critical/10"
      : recommendation.priority === "interesting"
      ? "text-lunar-warning border-lunar-warning/30 bg-lunar-warning/10"
      : "text-lunar-muted border-lunar-border bg-lunar-surface";

  return (
    <div className="p-3 rounded-lg bg-lunar-bg/60 border border-lunar-border/80 space-y-2.5">
      {showRecommendations ? (
        <>
          <div className="flex items-center justify-between">
            <span className="text-[10px] uppercase tracking-[0.14em] text-lunar-muted font-medium">
              Recommendation
            </span>
            {recommendation.metricHighlight && (
              <span
                className={`text-[10px] px-1.5 py-0.2 rounded border ${priorityBadge}`}
              >
                {recommendation.metricHighlight}
              </span>
            )}
          </div>

          <p className="text-xs text-lunar-text leading-relaxed line-clamp-2">
            {recommendation.message}
          </p>
        </>
      ) : (
        <div className="flex items-center justify-between">
          <span className="text-xs text-lunar-text-sec">
            Contextual AI assistant ready
          </span>
        </div>
      )}

      <div className="flex items-center gap-2 pt-0.5">
        <button
          type="button"
          onClick={handleViewProcess}
          className="flex-1 h-7 px-2.5 rounded-md bg-lunar-surface hover:bg-lunar-elevated border border-lunar-border text-xs text-lunar-text-sec hover:text-lunar-white flex items-center justify-center gap-1.5 transition-colors cursor-pointer"
        >
          <ExternalLink className="w-3 h-3" />
          <span>View</span>
        </button>

        <button
          type="button"
          onClick={handleAskAboutRecommendation}
          className="flex-1 h-7 px-2.5 rounded-md bg-lunar-elevated hover:bg-lunar-border border border-lunar-ai/30 text-xs font-medium text-lunar-white flex items-center justify-center gap-1.5 transition-colors cursor-pointer"
        >
          <Sparkles className="w-3 h-3 text-lunar-ai" />
          <span>Ask Asao</span>
        </button>
      </div>
    </div>
  );
};
