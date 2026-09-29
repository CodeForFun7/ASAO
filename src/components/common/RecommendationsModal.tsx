import React, { useEffect, useState, useCallback } from "react";
import {
  X,
  ExternalLink,
  CheckCircle2,
  RefreshCw,
  FolderOpen,
  PowerOff,
  Search,
} from "lucide-react";
import type {
  RecommendationCategory,
  WidgetRecommendation,
} from "../../types/widget";
import { getCategorizedRecommendations } from "../../services/widget";

interface RecButtonAndModalProps {
  category: RecommendationCategory;
  fallbackRecommendations?: WidgetRecommendation[];
  onInspectProcess?: (pid: number) => void;
  onInspectStartup?: (itemId: string) => void;
  onDisableStartup?: (itemId: string) => void;
  onInspectStoragePath?: (path: string) => void;
  onOpenStorageInExplorer?: (path: string) => void;
}

export const RecButtonAndModal: React.FC<RecButtonAndModalProps> = ({
  category,
  fallbackRecommendations = [],
  onInspectProcess,
  onInspectStartup,
  onDisableStartup,
  onInspectStoragePath,
  onOpenStorageInExplorer,
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const [rustRecommendations, setRustRecommendations] = useState<
    WidgetRecommendation[]
  >([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetchRecommendations = useCallback(async () => {
    setIsLoading(true);
    try {
      const res = await getCategorizedRecommendations();
      if (category === "process") {
        setRustRecommendations(res.processRecommendations || []);
      } else if (category === "startup") {
        setRustRecommendations(res.startupRecommendations || []);
      } else if (category === "storage") {
        setRustRecommendations(res.storageRecommendations || []);
      }
    } catch {
      // Fallback to local recommendations if running in browser preview
    } finally {
      setIsLoading(false);
    }
  }, [category]);

  useEffect(() => {
    void fetchRecommendations();
    const interval = window.setInterval(() => {
      void fetchRecommendations();
    }, 10000);
    return () => window.clearInterval(interval);
  }, [fetchRecommendations]);

  // Refresh immediately when modal opens
  useEffect(() => {
    if (isOpen) {
      void fetchRecommendations();
    }
  }, [isOpen, fetchRecommendations]);

  const activeRecommendations =
    rustRecommendations.length > 0
      ? rustRecommendations
      : fallbackRecommendations;

  const count = activeRecommendations.length;

  const getHeaderMeta = () => {
    switch (category) {
      case "process":
        return {
          title: "Process Recommendations",
          subtitle:
            "Native Rust inference identifying high-resource idle background processes with no user activity in the past 30+ minutes.",
        };
      case "startup":
        return {
          title: "Startup Recommendations",
          subtitle:
            "Native Rust inference identifying heavy startup entries that are not core Windows programs or needed at boot.",
        };
      case "storage":
        return {
          title: "Storage Recommendations",
          subtitle:
            "Native Rust inference identifying large dormant user files (1+ year inactive) and residual files from uninstalled applications.",
        };
    }
  };

  const meta = getHeaderMeta();

  return (
    <>
      {/* REC Button with Top-Left Count Badge (lunar-white number, no icon) */}
      <button
        type="button"
        onClick={() => setIsOpen(true)}
        className="relative inline-flex items-center justify-center h-9 px-3.5 rounded-lg bg-lunar-elevated hover:bg-lunar-border text-xs font-semibold text-lunar-white border border-lunar-border/90 hover:border-lunar-white/40 transition-all cursor-pointer select-none shadow-sm shrink-0"
        title={`View ${meta.title} (${count})`}
      >
        {/* Badge at Top-Left showing recommendation numbers in lunar-white */}
        <span className="absolute -top-2 -left-2 min-w-[19px] h-[19px] px-1 rounded-full bg-lunar-surface border border-lunar-border text-lunar-white text-[10px] font-mono font-bold flex items-center justify-center shadow-md z-10 leading-none">
          {count}
        </span>

        <span className="tracking-wider">REC</span>
      </button>

      {/* Recommendations Modal */}
      {isOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm animate-in fade-in duration-150"
          onClick={() => setIsOpen(false)}
        >
          <div
            className="w-full max-w-2xl max-h-[82vh] rounded-xl bg-[#111114] border border-lunar-border shadow-2xl flex flex-col overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Modal Header (without icon or recommendation number badge) */}
            <div className="px-5 py-4 border-b border-lunar-border bg-lunar-surface flex items-start justify-between gap-4 shrink-0">
              <div>
                <h2 className="text-sm font-semibold text-lunar-white tracking-tight">
                  {meta.title}
                </h2>
                <p className="text-xs text-lunar-text-sec mt-1 leading-relaxed">
                  {meta.subtitle}
                </p>
              </div>

              <div className="flex items-center gap-1.5 shrink-0">
                <button
                  type="button"
                  onClick={() => void fetchRecommendations()}
                  className="p-1.5 rounded-lg text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
                  title="Refresh Rust Engine Inference"
                >
                  <RefreshCw
                    className={`w-3.5 h-3.5 ${isLoading ? "animate-spin" : ""}`}
                  />
                </button>
                <button
                  type="button"
                  onClick={() => setIsOpen(false)}
                  className="p-1.5 rounded-lg text-lunar-text-sec hover:text-lunar-white hover:bg-lunar-elevated transition-colors cursor-pointer"
                  title="Close"
                >
                  <X className="w-4 h-4" />
                </button>
              </div>
            </div>

            {/* Modal Body: Recommendations List */}
            <div className="flex-1 overflow-y-auto p-5 space-y-3 min-h-0">
              {activeRecommendations.length === 0 ? (
                <div className="py-12 text-center space-y-2">
                  <CheckCircle2 className="w-8 h-8 text-lunar-healthy mx-auto stroke-[1.5]" />
                  <p className="text-sm font-medium text-lunar-white">
                    No optimization issues detected right now
                  </p>
                  <p className="text-xs text-lunar-text-sec max-w-md mx-auto">
                    All {category} activity is currently operating within optimal bounds.
                  </p>
                </div>
              ) : (
                activeRecommendations.map((rec) => (
                  <div
                    key={rec.id}
                    className="p-4 rounded-xl bg-lunar-surface/90 hover:bg-lunar-elevated/60 border border-lunar-border transition-colors space-y-2.5"
                  >
                    {/* Top Meta Row */}
                    <div className="flex items-center justify-between gap-2 flex-wrap">
                      <div className="flex items-center gap-2">
                        <span
                          className={`px-2 py-0.5 rounded text-[10px] font-mono uppercase tracking-wider font-semibold border ${
                            rec.priority === "important"
                              ? "bg-amber-500/15 text-amber-300 border-amber-500/30"
                              : "bg-lunar-ai/15 text-lunar-ai border-lunar-ai/30"
                          }`}
                        >
                          {rec.priority === "important"
                            ? "High Priority"
                            : "Recommended"}
                        </span>

                        {rec.subCategory && (
                          <span className="text-[11px] font-medium text-lunar-text-sec">
                            {rec.subCategory}
                          </span>
                        )}
                      </div>

                      {rec.metricHighlight && (
                        <span className="px-2 py-0.5 rounded bg-lunar-bg border border-lunar-border text-[11px] font-mono text-lunar-white">
                          {rec.metricHighlight}
                        </span>
                      )}
                    </div>

                    {/* Title & Detailed Inference Explanation */}
                    <div>
                      <h3 className="text-xs font-semibold text-lunar-white">
                        {rec.title}
                      </h3>
                      <p className="text-xs text-lunar-text-sec leading-relaxed mt-1">
                        {rec.message}
                      </p>
                    </div>

                    {/* Storage Path Pill if applicable */}
                    {rec.storagePath && (
                      <div className="px-2.5 py-1.5 rounded bg-lunar-bg/80 border border-lunar-border/70 font-mono text-[11px] text-lunar-muted truncate">
                        {rec.storagePath}
                      </div>
                    )}

                    {/* Action Footer */}
                    <div className="pt-1 flex items-center justify-between gap-2 border-t border-lunar-border/50">
                      <span className="text-[11px] text-lunar-ai font-medium">
                        {rec.actionLabel || "Recommended optimization"}
                      </span>

                      <div className="flex items-center gap-2">
                        {category === "process" &&
                          rec.processPid &&
                          onInspectProcess && (
                            <button
                              type="button"
                              onClick={() => {
                                onInspectProcess(rec.processPid!);
                                setIsOpen(false);
                              }}
                              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-lunar-elevated hover:bg-lunar-border text-[11px] font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
                            >
                              <Search className="w-3 h-3" />
                              <span>Inspect Process</span>
                            </button>
                          )}

                        {category === "startup" && (
                          <>
                            {rec.startupItemId && onDisableStartup && (
                              <button
                                type="button"
                                onClick={() => {
                                  onDisableStartup(rec.startupItemId!);
                                }}
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-500/15 hover:bg-amber-500/25 text-[11px] font-medium text-amber-200 border border-amber-500/30 transition-colors cursor-pointer"
                              >
                                <PowerOff className="w-3 h-3" />
                                <span>Disable at Boot</span>
                              </button>
                            )}
                            {rec.startupItemId && onInspectStartup && (
                              <button
                                type="button"
                                onClick={() => {
                                  onInspectStartup(rec.startupItemId!);
                                  setIsOpen(false);
                                }}
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-lunar-elevated hover:bg-lunar-border text-[11px] font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
                              >
                                <ExternalLink className="w-3 h-3" />
                                <span>Details</span>
                              </button>
                            )}
                          </>
                        )}

                        {category === "storage" && rec.storagePath && (
                          <>
                            {onOpenStorageInExplorer && (
                              <button
                                type="button"
                                onClick={() =>
                                  onOpenStorageInExplorer(rec.storagePath!)
                                }
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-lunar-elevated hover:bg-lunar-border text-[11px] font-medium text-lunar-white border border-lunar-border transition-colors cursor-pointer"
                              >
                                <FolderOpen className="w-3 h-3" />
                                <span>Review in Explorer</span>
                              </button>
                            )}
                            {onInspectStoragePath && (
                              <button
                                type="button"
                                onClick={() => {
                                  onInspectStoragePath(rec.storagePath!);
                                  setIsOpen(false);
                                }}
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-lunar-surface hover:bg-lunar-elevated text-[11px] font-medium text-lunar-text-sec hover:text-lunar-white border border-lunar-border transition-colors cursor-pointer"
                              >
                                <ExternalLink className="w-3 h-3" />
                                <span>Locate in ASAO</span>
                              </button>
                            )}
                          </>
                        )}
                      </div>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
};
