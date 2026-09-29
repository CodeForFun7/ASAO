import React, { useState } from "react";
import {
  CheckCircle2,
  Sparkles,
  ArrowRight,
  FolderOpen,
  FileText,
} from "lucide-react";
import {
  STORAGE_IMPORTANCE_META,
  type StorageInsight,
  type StorageInsightAffectedItem,
} from "../../types/storage";
import { formatStorageBytes } from "../../services/storage";

interface InsightCardProps {
  insight: StorageInsight;
  isSelected: boolean;
  onSelect: () => void;
  onReview: (insight: StorageInsight) => void;
}

export const InsightCard: React.FC<InsightCardProps> = ({
  insight,
  isSelected,
  onSelect,
  onReview,
}) => {
  const impMeta =
    STORAGE_IMPORTANCE_META[insight.importance] ??
    STORAGE_IMPORTANCE_META.NORMAL;

  return (
    <div
      onClick={onSelect}
      className={`rounded border px-3 py-2.5 transition-colors cursor-pointer ${
        isSelected
          ? "bg-lunar-elevated border-lunar-white/25"
          : "bg-lunar-bg/70 hover:bg-lunar-surface-2 border-lunar-border"
      }`}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-3 min-w-0">
          <span className="font-mono text-xs font-semibold text-lunar-white w-20 shrink-0">
            {formatStorageBytes(insight.size)}
          </span>
          <span className="text-xs font-medium text-lunar-text truncate">
            {insight.title}
          </span>
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <span
            className={`px-1.5 py-0.5 text-[10px] font-mono rounded border ${impMeta.badgeClass}`}
          >
            {impMeta.label}
          </span>
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onReview(insight);
            }}
            className="px-2 py-0.5 rounded bg-lunar-surface border border-lunar-border hover:border-lunar-white/30 text-[11px] font-mono text-lunar-white transition-colors cursor-pointer"
          >
            Review
          </button>
        </div>
      </div>

      {isSelected && (
        <div className="mt-2.5 pt-2 border-t border-lunar-border/60 space-y-2">
          <p className="text-[11px] text-lunar-text-sec leading-relaxed">
            {insight.description}
          </p>

          {insight.affectedItems.length > 0 && (
            <div className="space-y-1">
              <div className="text-[10px] font-mono uppercase tracking-[0.12em] text-lunar-muted">
                Top Detected Items
              </div>
              <div className="max-h-28 overflow-y-auto space-y-1 pr-1">
                {insight.affectedItems.slice(0, 5).map((item) => (
                  <div
                    key={item.path}
                    className="flex items-center justify-between gap-2 text-[11px] font-mono text-lunar-text-sec bg-lunar-surface px-2 py-1 rounded border border-lunar-border/60"
                  >
                    <span className="flex items-center gap-1.5 min-w-0 truncate">
                      {item.isDir ? (
                        <FolderOpen className="w-3 h-3 text-lunar-muted shrink-0" />
                      ) : (
                        <FileText className="w-3 h-3 text-lunar-muted shrink-0" />
                      )}
                      <span className="truncate text-lunar-text">
                        {item.name}
                      </span>
                    </span>
                    <span className="text-lunar-white shrink-0">
                      {formatStorageBytes(item.size)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

interface StorageInsightsProps {
  insights: StorageInsight[];
  driveUsagePercentage: number;
  activeInsightId: string | null;
  onReviewInsight: (insight: StorageInsight) => void;
  onInspectAffectedItem?: (item: StorageInsightAffectedItem) => void;
}

export const StorageInsights: React.FC<StorageInsightsProps> = ({
  insights,
  driveUsagePercentage,
  activeInsightId,
  onReviewInsight,
}) => {
  const [expandedId, setExpandedId] = useState<string | null>(
    insights[0]?.id ?? null
  );

  // Filter reclaimable insights vs capacity pressure insights for total calculation
  const reclaimableInsights = insights.filter(
    (i) => i.insightType !== "storage_critically_full"
  );
  const totalReclaimableBytes = reclaimableInsights.reduce(
    (acc, i) => acc + i.size,
    0
  );

  const primaryInsight =
    insights.find((i) => i.id === expandedId) ?? insights[0] ?? null;

  return (
    <div className="rounded-lg lunar-glass-card p-5 flex flex-col justify-between h-full">
      <div>
        {/* Header */}
        <div className="flex items-center justify-between gap-2 mb-4">
          <div className="flex items-center gap-2">
            <Sparkles className="w-3.5 h-3.5 text-lunar-ai" />
            <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-muted">
              ASAO Insight
            </h2>
          </div>
          {insights.length > 0 && (
            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-lunar-bg border border-lunar-border text-lunar-text-sec">
              {insights.length}{" "}
              {insights.length === 1 ? "finding" : "findings"}
            </span>
          )}
        </div>

        {insights.length === 0 ? (
          <div className="py-12 flex flex-col items-center justify-center text-center">
            <CheckCircle2 className="w-7 h-7 text-lunar-healthy mb-3 stroke-[1.5]" />
            <p className="text-sm font-medium text-lunar-white">
              No storage issues detected.
            </p>
            <p className="text-xs text-lunar-text-sec mt-1 max-w-xs">
              ASAO hasn't found anything requiring attention.
            </p>
          </div>
        ) : (
          <div className="space-y-4">
            {/* Dynamic Summary Headline */}
            <div className="space-y-1">
              <p className="text-sm font-semibold text-lunar-white">
                {driveUsagePercentage >= 80
                  ? "Storage is getting crowded."
                  : totalReclaimableBytes > 0
                  ? "Reclaimable storage footprint detected."
                  : primaryInsight?.title}
              </p>
              <p className="text-xs text-lunar-text-sec leading-relaxed">
                {totalReclaimableBytes > 0 ? (
                  <>
                    <span className="text-lunar-white font-mono font-medium">
                      {formatStorageBytes(totalReclaimableBytes)}
                    </span>{" "}
                    of potentially removable or cache data was detected across
                    this volume.
                  </>
                ) : (
                  primaryInsight?.description
                )}
              </p>
            </div>

            {/* Structured Insight Cards */}
            <div className="space-y-2">
              {insights.map((insight) => (
                <InsightCard
                  key={insight.id}
                  insight={insight}
                  isSelected={
                    expandedId === insight.id ||
                    activeInsightId === insight.id
                  }
                  onSelect={() =>
                    setExpandedId(
                      expandedId === insight.id ? null : insight.id
                    )
                  }
                  onReview={onReviewInsight}
                />
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Bottom Action Button */}
      {primaryInsight && (
        <div className="mt-4 pt-3 border-t border-lunar-border/60 flex items-center justify-between gap-3">
          <span className="text-[11px] font-mono text-lunar-muted truncate">
            Select an insight to inspect matching files below
          </span>
          <button
            type="button"
            onClick={() => onReviewInsight(primaryInsight)}
            className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded bg-lunar-elevated hover:bg-lunar-border text-xs font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer shrink-0"
          >
            <span>Review</span>
            <ArrowRight className="w-3.5 h-3.5" />
          </button>
        </div>
      )}
    </div>
  );
};
