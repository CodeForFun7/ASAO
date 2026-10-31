import React, { useCallback, useEffect, useRef, useState } from "react";
import { ChevronLeft, ChevronRight, CheckCircle2, Lightbulb } from "lucide-react";
import type {
  RecommendationCategory,
  WidgetRecommendation,
} from "../../types/widget";
import { getCategorizedRecommendations } from "../../services/widget";
import { useProcessStore } from "../../stores/process-store";
import { useStartupStore } from "../../stores/startup-store";
import { useStorageStore } from "../../stores/storage-store";

const SLIDE_INTERVAL_MS = 6000;
const REFRESH_INTERVAL_MS = 10000;

const CATEGORY_LABEL: Record<RecommendationCategory, string> = {
  process: "Process",
  startup: "Startup",
  storage: "Storage",
};

function categoryOf(rec: WidgetRecommendation): RecommendationCategory {
  if (rec.category) return rec.category;
  if (rec.startupItemId) return "startup";
  if (rec.storagePath || rec.id.startsWith("storage")) return "storage";
  return "process";
}

export const RecommendationCarousel: React.FC = () => {
  const setRoute = useProcessStore((s) => s.setRoute);
  const selectProcess = useProcessStore((s) => s.selectProcess);

  const [items, setItems] = useState<WidgetRecommendation[]>([]);
  const [index, setIndex] = useState(0);
  const [paused, setPaused] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const cycleRef = useRef(0);

  // Pull the combined list from the Rust recommendation engine.
  useEffect(() => {
    let alive = true;
    const load = () => {
      getCategorizedRecommendations()
        .then((res) => {
          if (!alive) return;
          setItems(res.combinedRecommendations ?? []);
          setLoaded(true);
        })
        .catch(() => {
          if (alive) setLoaded(true);
        });
    };
    load();
    const timer = window.setInterval(load, REFRESH_INTERVAL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);

  const count = items.length;
  const safeIndex = count === 0 ? 0 : Math.min(index, count - 1);

  // Auto-advance. `cycleRef` restarts the interval after a manual change.
  useEffect(() => {
    if (count < 2 || paused) return;
    const timer = window.setInterval(() => {
      setIndex((i) => (i + 1) % count);
    }, SLIDE_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [count, paused, index]);

  const goTo = useCallback(
    (next: number) => {
      if (count === 0) return;
      cycleRef.current += 1;
      setIndex(((next % count) + count) % count);
    },
    [count]
  );

  const act = (rec: WidgetRecommendation) => {
    const category = categoryOf(rec);
    if (category === "process") {
      setRoute("processes");
      if (rec.processPid) selectProcess(rec.processPid);
    } else if (category === "startup") {
      setRoute("startup");
      if (rec.startupItemId) {
        useStartupStore.getState().selectItem(rec.startupItemId);
      }
    } else {
      if (rec.id.startsWith("storage-leftovers")) {
        useStorageStore.getState().setActiveView("leftovers");
      }
      setRoute("storage");
    }
  };

  return (
    <section
      className="h-full min-h-0 rounded-xl lunar-glass-card p-4 flex flex-col gap-3"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
      aria-roledescription="carousel"
      aria-label="Recommendations"
    >
      <header className="flex items-center justify-between gap-2 shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <Lightbulb className="w-3.5 h-3.5 text-lunar-ai shrink-0" />
          <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-text-sec truncate">
            Recommendations
          </h2>
        </div>
        {count > 1 && (
          <div className="flex items-center gap-1 shrink-0">
            <span className="text-[11px] font-mono text-lunar-muted mr-1">
              {safeIndex + 1} / {count}
            </span>
            <button
              type="button"
              onClick={() => goTo(safeIndex - 1)}
              className="p-1 rounded text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
              aria-label="Previous recommendation"
            >
              <ChevronLeft className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={() => goTo(safeIndex + 1)}
              className="p-1 rounded text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
              aria-label="Next recommendation"
            >
              <ChevronRight className="w-3.5 h-3.5" />
            </button>
          </div>
        )}
      </header>

      {count === 0 ? (
        <div className="flex-1 min-h-0 flex flex-col items-center justify-center text-center gap-2 px-2">
          <CheckCircle2 className="w-8 h-8 text-lunar-healthy stroke-[1.5]" />
          <p className="text-sm font-medium text-lunar-white">
            {loaded ? "Nothing needs attention" : "Analyzing your system..."}
          </p>
          {loaded && (
            <p className="text-xs text-lunar-text-sec max-w-xs leading-relaxed">
              Processes, startup apps and storage are all within normal bounds.
            </p>
          )}
        </div>
      ) : (
        <>
          {/* Sliding track: every slide is one full card width */}
          <div className="flex-1 min-h-0 overflow-hidden">
            <div
              className="flex h-full transition-transform duration-500 ease-out"
              style={{ transform: `translateX(-${safeIndex * 100}%)` }}
            >
              {items.map((rec, i) => (
                <article
                  key={rec.id}
                  className="w-full shrink-0 h-full flex flex-col gap-2 min-h-0 pr-px"
                  aria-hidden={i !== safeIndex}
                  aria-label={`${i + 1} of ${count}`}
                >
                  <div className="flex items-center gap-2 flex-wrap shrink-0">
                    <span
                      className={`px-2 py-0.5 rounded text-[10px] font-mono uppercase tracking-wider font-semibold border ${
                        rec.priority === "important"
                          ? "bg-amber-500/15 text-amber-300 border-amber-500/30"
                          : "bg-lunar-ai/15 text-lunar-ai border-lunar-ai/30"
                      }`}
                    >
                      {rec.priority === "important" ? "High priority" : "Recommended"}
                    </span>
                    <span className="text-[11px] font-medium text-lunar-text-sec">
                      {CATEGORY_LABEL[categoryOf(rec)]}
                      {rec.subCategory ? ` · ${rec.subCategory}` : ""}
                    </span>
                  </div>

                  <h3 className="text-sm font-semibold text-lunar-white leading-snug shrink-0 line-clamp-2">
                    {rec.title}
                  </h3>

                  <p className="text-xs text-lunar-text-sec leading-relaxed flex-1 min-h-0 overflow-hidden">
                    {rec.message}
                  </p>

                  <div className="flex items-center justify-between gap-2 shrink-0">
                    {rec.metricHighlight ? (
                      <span className="px-2 py-0.5 rounded bg-lunar-bg border border-lunar-border text-[11px] font-mono text-lunar-white truncate">
                        {rec.metricHighlight}
                      </span>
                    ) : (
                      <span />
                    )}
                    <button
                      type="button"
                      tabIndex={i === safeIndex ? 0 : -1}
                      onClick={() => act(rec)}
                      className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-lunar-elevated hover:bg-lunar-border text-[11px] font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer shrink-0"
                    >
                      <span>{rec.actionLabel || "Review"}</span>
                      <ChevronRight className="w-3 h-3" />
                    </button>
                  </div>
                </article>
              ))}
            </div>
          </div>

          {/* Dots + progress */}
          {count > 1 && (
            <div className="flex items-center justify-center gap-1.5 shrink-0">
              {items.map((rec, i) => (
                <button
                  key={rec.id}
                  type="button"
                  onClick={() => goTo(i)}
                  aria-label={`Go to recommendation ${i + 1}`}
                  className={`h-1.5 rounded-full transition-all duration-300 cursor-pointer ${
                    i === safeIndex
                      ? "w-5 bg-lunar-white"
                      : "w-1.5 bg-lunar-border hover:bg-lunar-muted"
                  }`}
                />
              ))}
            </div>
          )}
        </>
      )}
    </section>
  );
};
