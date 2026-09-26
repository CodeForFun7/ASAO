import React, { useMemo } from "react";
import {
  CATEGORY_METADATA,
  type ProcessCategory,
  type ProcessInfo,
} from "../../types/process";

interface ProcessCategoryTabsProps {
  processes: ProcessInfo[];
  activeCategory: ProcessCategory | "all";
  onSelectCategory: (category: ProcessCategory | "all") => void;
}

const ORDERED_CATEGORIES: Array<ProcessCategory | "all"> = [
  "all",
  "windows-core",
  "drivers",
  "gaming",
  "development",
  "productivity",
  "communication",
  "browser",
  "unknown",
];

export const ProcessCategoryTabs: React.FC<ProcessCategoryTabsProps> = ({
  processes,
  activeCategory,
  onSelectCategory,
}) => {
  const counts = useMemo(() => {
    const map: Record<ProcessCategory | "all", number> = {
      all: processes.length,
      "windows-core": 0,
      drivers: 0,
      gaming: 0,
      development: 0,
      productivity: 0,
      communication: 0,
      browser: 0,
      unknown: 0,
    };
    for (const p of processes) {
      if (map[p.category] !== undefined) {
        map[p.category] += 1;
      } else {
        map.unknown += 1;
      }
    }
    return map;
  }, [processes]);

  return (
    <div className="grid grid-cols-3 sm:grid-cols-5 lg:grid-cols-9 gap-2">
      {ORDERED_CATEGORIES.map((cat) => {
        const isSelected = activeCategory === cat;
        const meta = CATEGORY_METADATA[cat];
        const count = counts[cat] ?? 0;

        return (
          <button
            key={cat}
            type="button"
            onClick={() => onSelectCategory(cat)}
            title={meta.description}
            className={`text-left px-3 py-2 rounded-xl transition-all cursor-pointer flex flex-col justify-between min-h-[54px] ${
              isSelected
                ? "lunar-glass-card border-lunar-white/30 text-lunar-white"
                : "lunar-glass-card-interactive text-lunar-text-sec hover:text-lunar-text"
            }`}
          >
            <span className="text-[11px] font-medium truncate w-full">
              {meta.shortLabel}
            </span>
            <span
              className={`font-mono text-sm font-semibold ${
                isSelected ? "text-lunar-white" : "text-lunar-text"
              }`}
            >
              {count}
            </span>
          </button>
        );
      })}
    </div>
  );
};
