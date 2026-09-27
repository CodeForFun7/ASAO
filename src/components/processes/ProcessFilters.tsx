import React, { useState, useRef, useEffect } from "react";
import {
  Search,
  SlidersHorizontal,
  RotateCcw,
  Check,
  X,
} from "lucide-react";
import {
  CATEGORY_METADATA,
  STATUS_METADATA,
  type CriticalityFilter,
  type ProcessCategory,
  type ProcessStatus,
  type ResourceUsageFilter,
  type SortOption,
} from "../../types/process";

interface ProcessFiltersProps {
  searchQuery: string;
  onSearchChange: (q: string) => void;
  multiCategoryFilter: ProcessCategory[];
  onToggleCategory: (cat: ProcessCategory) => void;
  statusFilter: ProcessStatus[];
  onToggleStatus: (status: ProcessStatus) => void;
  resourceFilter: ResourceUsageFilter;
  onResourceFilterChange: (rf: ResourceUsageFilter) => void;
  criticalityFilter: CriticalityFilter;
  onCriticalityFilterChange: (cf: CriticalityFilter) => void;
  sort: SortOption;
  onSortChange: (sort: SortOption) => void;
  onResetAll: () => void;
}

const FILTERABLE_CATEGORIES: ProcessCategory[] = [
  "windows-core",
  "drivers",
  "gaming",
  "development",
  "productivity",
  "communication",
  "browser",
  "unknown",
];

const FILTERABLE_STATUSES: ProcessStatus[] = [
  "active",
  "background",
  "high-resource",
  "attention",
  "protected",
  "normal",
];

const RESOURCE_USAGE_SORT_OPTIONS: Array<{ value: SortOption; label: string }> = [
  { value: "resource-desc", label: "Highest Resource Usage" },
  { value: "cpu-desc", label: "CPU: Highest to Lowest" },
  { value: "cpu-asc", label: "CPU: Lowest to Highest" },
  { value: "memory-desc", label: "Memory: Highest to Lowest" },
  { value: "memory-asc", label: "Memory: Lowest to Highest" },
];

export const ProcessFilters: React.FC<ProcessFiltersProps> = ({
  searchQuery,
  onSearchChange,
  multiCategoryFilter,
  onToggleCategory,
  statusFilter,
  onToggleStatus,
  resourceFilter,
  onResourceFilterChange,
  criticalityFilter,
  onCriticalityFilterChange,
  sort,
  onSortChange,
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
    multiCategoryFilter.length +
    statusFilter.length +
    (resourceFilter !== "any" ? 1 : 0) +
    (sort !== "resource-desc" ? 1 : 0) +
    (criticalityFilter !== "all" ? 1 : 0);

  return (
    <div className="flex flex-wrap items-center justify-between gap-2.5">
      {/* Left: Search Processes */}
      <div className="relative flex-1 min-w-[240px] max-w-md">
        <Search className="w-3.5 h-3.5 text-lunar-muted absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          placeholder="Search processes by name, publisher, product, path..."
          className="w-full h-9 pl-9 pr-8 bg-lunar-surface border border-lunar-border rounded-md text-xs text-lunar-text placeholder:text-lunar-muted focus:outline-none focus:border-lunar-text-sec transition-colors"
        />
        {searchQuery && (
          <button
            type="button"
            onClick={() => onSearchChange("")}
            className="absolute right-2.5 top-1/2 -translate-y-1/2 text-lunar-muted hover:text-lunar-text"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      {/* Right: Filter Dropdown Only */}
      <div className="flex items-center gap-2">
        {activeFilterCount > 0 && (
          <button
            type="button"
            onClick={onResetAll}
            className="h-9 px-2.5 rounded-md bg-lunar-surface-2 hover:bg-lunar-elevated border border-lunar-border text-xs text-lunar-text-sec hover:text-lunar-white flex items-center gap-1.5 transition-colors cursor-pointer"
            title="Clear all active filters"
          >
            <RotateCcw className="w-3 h-3" />
            <span>Reset ({activeFilterCount})</span>
          </button>
        )}

        {/* Filter Menu */}
        <div className="relative" ref={filterRef}>
          <button
            type="button"
            onClick={() => setFilterOpen((prev) => !prev)}
            className={`h-9 px-3 rounded-md border text-xs font-medium flex items-center gap-2 transition-colors cursor-pointer ${
              filterOpen || activeFilterCount > 0
                ? "bg-lunar-elevated border-lunar-text-sec/50 text-lunar-white"
                : "bg-lunar-surface border-lunar-border text-lunar-text-sec hover:text-lunar-text hover:bg-lunar-surface-2"
            }`}
          >
            <SlidersHorizontal className="w-3.5 h-3.5" />
            <span>Filter</span>
            {activeFilterCount > 0 && (
              <span className="px-1.5 py-0.2 rounded bg-lunar-white/15 text-lunar-white font-mono text-[10px]">
                {activeFilterCount}
              </span>
            )}
          </button>

          {filterOpen && (
            <div className="absolute right-0 mt-1.5 w-80 rounded-xl bg-[#101318] border border-[#262C34] shadow-2xl z-50 p-4 space-y-4 text-xs">
              {/* Category Checkboxes */}
              <div>
                <div className="text-[10px] font-mono uppercase tracking-wider text-lunar-muted mb-2">
                  Category
                </div>
                <div className="grid grid-cols-2 gap-1.5">
                  {FILTERABLE_CATEGORIES.map((cat) => {
                    const checked = multiCategoryFilter.includes(cat);
                    return (
                      <label
                        key={cat}
                        onClick={() => onToggleCategory(cat)}
                        className="flex items-center gap-2 px-2 py-1 rounded hover:bg-lunar-surface-2 cursor-pointer"
                      >
                        <span
                          className={`w-3.5 h-3.5 rounded border flex items-center justify-center ${
                            checked
                              ? "bg-lunar-white border-lunar-white text-lunar-bg"
                              : "border-lunar-border bg-lunar-bg"
                          }`}
                        >
                          {checked && <Check className="w-2.5 h-2.5 stroke-[3]" />}
                        </span>
                        <span className="text-lunar-text truncate text-[11px]">
                          {CATEGORY_METADATA[cat].label}
                        </span>
                      </label>
                    );
                  })}
                </div>
              </div>

              {/* Resource Usage (Highest to Lowest CPU / Memory Options) */}
              <div className="border-t border-lunar-border pt-3">
                <div className="text-[10px] font-mono uppercase tracking-wider text-lunar-muted mb-2">
                  Resource Usage
                </div>
                <div className="space-y-1">
                  {RESOURCE_USAGE_SORT_OPTIONS.map((opt) => {
                    const selected = sort === opt.value;
                    return (
                      <label
                        key={opt.value}
                        onClick={() => {
                          onResourceFilterChange("any");
                          onSortChange(opt.value);
                        }}
                        className="flex items-center gap-2.5 px-2 py-1.5 rounded hover:bg-lunar-surface-2 cursor-pointer"
                      >
                        <span
                          className={`w-3.5 h-3.5 rounded-full border flex items-center justify-center shrink-0 ${
                            selected
                              ? "border-lunar-white"
                              : "border-lunar-border bg-lunar-bg"
                          }`}
                        >
                          {selected && (
                            <span className="w-1.5 h-1.5 rounded-full bg-lunar-white" />
                          )}
                        </span>
                        <span className="text-lunar-text text-[11px]">
                          {opt.label}
                        </span>
                      </label>
                    );
                  })}
                </div>
              </div>

              {/* Status Checkboxes */}
              <div className="border-t border-lunar-border pt-3">
                <div className="text-[10px] font-mono uppercase tracking-wider text-lunar-muted mb-2">
                  Status
                </div>
                <div className="grid grid-cols-2 gap-1.5">
                  {FILTERABLE_STATUSES.map((st) => {
                    const checked = statusFilter.includes(st);
                    return (
                      <label
                        key={st}
                        onClick={() => onToggleStatus(st)}
                        className="flex items-center gap-2 px-2 py-1 rounded hover:bg-lunar-surface-2 cursor-pointer"
                      >
                        <span
                          className={`w-3.5 h-3.5 rounded border flex items-center justify-center ${
                            checked
                              ? "bg-lunar-white border-lunar-white text-lunar-bg"
                              : "border-lunar-border bg-lunar-bg"
                          }`}
                        >
                          {checked && <Check className="w-2.5 h-2.5 stroke-[3]" />}
                        </span>
                        <span className="text-lunar-text truncate text-[11px]">
                          {STATUS_METADATA[st].label}
                        </span>
                      </label>
                    );
                  })}
                </div>
              </div>

              {/* System Criticality */}
              <div className="border-t border-lunar-border pt-3">
                <div className="text-[10px] font-mono uppercase tracking-wider text-lunar-muted mb-2">
                  System Criticality
                </div>
                <div className="flex items-center gap-1.5">
                  {(
                    [
                      { value: "all", label: "All" },
                      { value: "user-space", label: "User / Standard" },
                      { value: "system-critical", label: "Protected Only" },
                    ] as const
                  ).map((item) => (
                    <button
                      key={item.value}
                      type="button"
                      onClick={() => onCriticalityFilterChange(item.value)}
                      className={`flex-1 py-1 px-2 rounded border text-[11px] font-mono cursor-pointer ${
                        criticalityFilter === item.value
                          ? "bg-lunar-elevated border-lunar-text-sec text-lunar-white"
                          : "bg-lunar-bg border-lunar-border text-lunar-text-sec"
                      }`}
                    >
                      {item.label}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

