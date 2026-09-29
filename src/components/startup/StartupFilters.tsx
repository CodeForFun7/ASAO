import React, { useState, useRef, useEffect } from "react";
import {
  Search,
  SlidersHorizontal,
  RotateCcw,
  Check,
  X,
  Activity,
  RefreshCw,
} from "lucide-react";
import type { Recommendation, StartupImpact } from "../../types/startup";

interface StartupFiltersProps {
  searchQuery: string;
  onSearchChange: (q: string) => void;
  impactFilter: StartupImpact | "all";
  onImpactChange: (i: StartupImpact | "all") => void;
  recommendationFilter: Recommendation | "all";
  onRecommendationChange: (r: Recommendation | "all") => void;
  runningFilter: "all" | "running" | "stopped";
  onRunningChange: (r: "all" | "running" | "stopped") => void;
  isLoading: boolean;
  onRefresh: () => void;
  onOpenWprModal: () => void;
  onResetAll: () => void;
}

export const StartupFilters: React.FC<StartupFiltersProps> = ({
  searchQuery,
  onSearchChange,
  impactFilter,
  onImpactChange,
  recommendationFilter,
  onRecommendationChange,
  runningFilter,
  onRunningChange,
  isLoading,
  onRefresh,
  onOpenWprModal,
  onResetAll,
}) => {
  const [filterOpen, setFilterOpen] = useState(false);
  const filterRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleOutside = (e: MouseEvent) => {
      if (filterRef.current && !filterRef.current.contains(e.target as Node)) {
        setFilterOpen(false);
      }
    };
    document.addEventListener("mousedown", handleOutside);
    return () => document.removeEventListener("mousedown", handleOutside);
  }, []);

  const activeFilterCount =
    (impactFilter !== "all" ? 1 : 0) +
    (recommendationFilter !== "all" ? 1 : 0) +
    (runningFilter !== "all" ? 1 : 0);

  return (
    <div className="flex flex-wrap items-center justify-between gap-2.5">
      {/* Left: Search Bar matching ProcessFilters */}
      <div className="relative flex-1 min-w-[240px] max-w-md">
        <Search className="w-3.5 h-3.5 text-lunar-muted absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Search startup items by name, publisher, path..."
          className="w-full pl-9 pr-8 py-2 rounded-lg bg-lunar-surface-2 border border-lunar-border text-xs text-lunar-text placeholder:text-lunar-muted focus:outline-hidden focus:border-lunar-white/30 transition-colors"
        />
        {searchQuery && (
          <button
            type="button"
            onClick={() => onSearchChange("")}
            className="absolute right-2.5 top-1/2 -translate-y-1/2 text-lunar-muted hover:text-lunar-white cursor-pointer"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      {/* Right Controls: Filter popover, WPR Boot Trace, Refresh */}
      <div className="flex items-center gap-2">
        {/* Filter Popover Button matching ProcessFilters */}
        <div className="relative" ref={filterRef}>
          <button
            type="button"
            onClick={() => setFilterOpen((v) => !v)}
            className={`inline-flex items-center gap-2 px-3 py-2 rounded-lg text-xs font-medium border transition-colors cursor-pointer ${
              activeFilterCount > 0
                ? "bg-lunar-elevated border-lunar-white/30 text-lunar-white"
                : "bg-lunar-surface-2 border-lunar-border text-lunar-text-sec hover:text-lunar-text"
            }`}
          >
            <SlidersHorizontal className="w-3.5 h-3.5" />
            <span>Filter</span>
            {activeFilterCount > 0 && (
              <span className="w-4 h-4 rounded-full bg-lunar-white text-black font-mono text-[10px] flex items-center justify-center font-bold">
                {activeFilterCount}
              </span>
            )}
          </button>

          {/* Filter Popover Content */}
          {filterOpen && (
            <div className="absolute right-0 mt-2 w-72 rounded-xl lunar-glass-card p-4 z-50 shadow-2xl space-y-4">
              <div className="flex items-center justify-between pb-2 border-b border-lunar-border">
                <span className="text-xs font-semibold text-lunar-white">
                  Filter Startup Entries
                </span>
                {activeFilterCount > 0 && (
                  <button
                    type="button"
                    onClick={() => {
                      onResetAll();
                      setFilterOpen(false);
                    }}
                    className="text-[11px] text-lunar-ai hover:underline flex items-center gap-1 cursor-pointer"
                  >
                    <RotateCcw className="w-3 h-3" />
                    <span>Reset</span>
                  </button>
                )}
              </div>

              {/* Impact Filter */}
              <div className="space-y-1.5">
                <label className="text-[10px] font-mono uppercase tracking-wider text-lunar-muted">
                  Startup Impact
                </label>
                <div className="grid grid-cols-2 gap-1.5">
                  {(
                    [
                      { id: "all", label: "All Impact" },
                      { id: "high", label: "High" },
                      { id: "medium", label: "Medium" },
                      { id: "low", label: "Low" },
                    ] as const
                  ).map((opt) => (
                    <button
                      key={opt.id}
                      type="button"
                      onClick={() => onImpactChange(opt.id)}
                      className={`px-2 py-1 rounded text-xs text-left transition-colors cursor-pointer flex items-center justify-between ${
                        impactFilter === opt.id
                          ? "bg-lunar-elevated text-lunar-white border border-lunar-border"
                          : "text-lunar-text-sec hover:text-lunar-text hover:bg-lunar-surface-2"
                      }`}
                    >
                      <span>{opt.label}</span>
                      {impactFilter === opt.id && (
                        <Check className="w-3 h-3 text-lunar-white" />
                      )}
                    </button>
                  ))}
                </div>
              </div>

              {/* Recommendation Filter */}
              <div className="space-y-1.5">
                <label className="text-[10px] font-mono uppercase tracking-wider text-lunar-muted">
                  Recommendation
                </label>
                <div className="space-y-1">
                  {(
                    [
                      { id: "all", label: "All Recommendations" },
                      { id: "disable", label: "Disable Recommended" },
                      { id: "investigate", label: "Investigate" },
                      { id: "keep", label: "Keep / Essential" },
                    ] as const
                  ).map((opt) => (
                    <button
                      key={opt.id}
                      type="button"
                      onClick={() => onRecommendationChange(opt.id)}
                      className={`w-full px-2 py-1 rounded text-xs text-left transition-colors cursor-pointer flex items-center justify-between ${
                        recommendationFilter === opt.id
                          ? "bg-lunar-elevated text-lunar-white border border-lunar-border"
                          : "text-lunar-text-sec hover:text-lunar-text hover:bg-lunar-surface-2"
                      }`}
                    >
                      <span>{opt.label}</span>
                      {recommendationFilter === opt.id && (
                        <Check className="w-3 h-3 text-lunar-white" />
                      )}
                    </button>
                  ))}
                </div>
              </div>

              {/* Activity Filter */}
              <div className="space-y-1.5">
                <label className="text-[10px] font-mono uppercase tracking-wider text-lunar-muted">
                  Process Activity
                </label>
                <div className="grid grid-cols-3 gap-1">
                  {(
                    [
                      { id: "all", label: "All" },
                      { id: "running", label: "Running" },
                      { id: "stopped", label: "Inactive" },
                    ] as const
                  ).map((opt) => (
                    <button
                      key={opt.id}
                      type="button"
                      onClick={() => onRunningChange(opt.id)}
                      className={`px-2 py-1 rounded text-xs text-center transition-colors cursor-pointer ${
                        runningFilter === opt.id
                          ? "bg-lunar-elevated text-lunar-white border border-lunar-border font-medium"
                          : "text-lunar-text-sec hover:text-lunar-text hover:bg-lunar-surface-2"
                      }`}
                    >
                      <span>{opt.label}</span>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* WPR Boot Trace Trigger */}
        <button
          type="button"
          onClick={onOpenWprModal}
          title="Windows Performance Recorder & ETW Boot Tracing"
          className="inline-flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-lunar-surface-2 hover:bg-lunar-elevated text-lunar-text-sec hover:text-lunar-white border border-lunar-border transition-colors cursor-pointer"
        >
          <Activity className="w-3.5 h-3.5 text-lunar-white" />
          <span className="hidden sm:inline">WPR Trace</span>
        </button>

        {/* Refresh Scan Button */}
        <button
          type="button"
          disabled={isLoading}
          onClick={onRefresh}
          title="Refresh startup scan"
          className="inline-flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-lunar-elevated hover:bg-lunar-border text-lunar-white border border-lunar-border transition-colors cursor-pointer disabled:opacity-50"
        >
          <RefreshCw
            className={`w-3.5 h-3.5 ${isLoading ? "animate-spin" : ""}`}
          />
          <span className="hidden sm:inline">
            {isLoading ? "Scanning..." : "Scan"}
          </span>
        </button>
      </div>
    </div>
  );
};
