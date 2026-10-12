import React, { useEffect, useState } from "react";
import { ExternalLink } from "lucide-react";
import type { WidgetRecommendation } from "../../types/widget";
import { openMainWindow } from "../../services/widget";

interface RecommendationCardProps {
  recommendations: WidgetRecommendation[];
}

const AUTO_SCROLL_INTERVAL_MS = 3800;

export const RecommendationCard: React.FC<RecommendationCardProps> = ({
  recommendations,
}) => {
  const items = recommendations.length > 0 ? recommendations : [];
  const [activeIndex, setActiveIndex] = useState(0);

  // Keep activeIndex within bounds when recommendation list length changes
  useEffect(() => {
    if (items.length === 0) {
      setActiveIndex(0);
    } else if (activeIndex >= items.length) {
      setActiveIndex(0);
    }
  }, [items.length, activeIndex]);

  // Auto-scroll through recommendations at a fixed interval
  useEffect(() => {
    if (items.length <= 1) return;
    const timer = window.setInterval(() => {
      setActiveIndex((prev) => (prev + 1) % items.length);
    }, AUTO_SCROLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [items.length]);

  const current = items[activeIndex] ?? items[0];

  const handleViewItem = () => {
    if (!current) return;
    if (current.category === "startup") {
      void openMainWindow("startup");
    } else if (current.category === "storage") {
      void openMainWindow("storage");
    } else {
      void openMainWindow("processes", current.processPid ?? undefined);
    }
  };

  const categoryBadge =
    current?.category === "startup"
      ? "STARTUP"
      : current?.category === "storage"
      ? "STORAGE"
      : "PROCESS";

  return (
    <div className="flex-1 min-h-0 p-3.5 rounded-lg bg-lunar-surface/55 backdrop-blur-sm border border-lunar-border/80 flex flex-col justify-between overflow-hidden">
      {/* Header with Notification Count Badge */}
      <div className="flex items-center justify-between shrink-0">
        <div className="flex items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.14em] text-lunar-muted font-medium">
            Recommendations
          </span>
          <span
            className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1.5 rounded-full bg-lunar-elevated/70 border border-lunar-border/80 text-[10px] font-semibold text-lunar-white leading-none"
            title={`${items.length} active recommendation${items.length === 1 ? "" : "s"}`}
          >
            {items.length}
          </span>
          {current && (
            <span className="px-1.5 py-0.5 rounded bg-lunar-elevated/70 border border-lunar-border/80 text-[9px] font-mono font-semibold text-lunar-white uppercase tracking-wider leading-none">
              {categoryBadge}
            </span>
          )}
        </div>

        {current?.metricHighlight && (
          <span className="text-[10px] font-medium text-lunar-text-sec truncate max-w-[115px]">
            {current.metricHighlight}
          </span>
        )}
      </div>

      {/* Auto-Scrolling Recommendation Viewport */}
      {current ? (
        <div
          key={current.id || activeIndex}
          className="my-auto py-1.5 space-y-1 transition-all duration-300"
        >
          <div className="flex items-center justify-between gap-2">
            <span className="text-xs font-semibold text-lunar-white truncate">
              {current.title}
            </span>
            <button
              type="button"
              onClick={handleViewItem}
              className="inline-flex items-center gap-1 text-[10px] text-lunar-text-sec hover:text-lunar-white transition-colors cursor-pointer shrink-0"
              title={`Open ${categoryBadge.toLowerCase()} in ASAO`}
            >
              <span>View</span>
              <ExternalLink className="w-2.5 h-2.5" />
            </button>
          </div>

          <p className="text-xs text-lunar-text-sec leading-relaxed line-clamp-2">
            {current.message}
          </p>
        </div>
      ) : (
        <div className="my-auto py-1.5 text-xs text-lunar-muted">
          Collecting system recommendations...
        </div>
      )}

      {/* Carousel Progress Dots */}
      {items.length > 1 && (
        <div className="flex items-center justify-center gap-1.5 pt-1 shrink-0">
          {items.map((rec, idx) => (
            <button
              key={rec.id || idx}
              type="button"
              onClick={() => setActiveIndex(idx)}
              className={`h-1 rounded-full transition-all duration-300 cursor-pointer ${
                idx === activeIndex
                  ? "w-4 bg-lunar-white"
                  : "w-1.5 bg-lunar-border hover:bg-lunar-muted"
              }`}
              title={`Recommendation ${idx + 1} of ${items.length}`}
            />
          ))}
        </div>
      )}
    </div>
  );
};
