import React from "react";
import { SearchX } from "lucide-react";
import type { StartupItem } from "../../types/startup";
import { StartupTableRow } from "./StartupTableRow";

interface StartupTableProps {
  items: StartupItem[];
  selectedId: string | null;
  onSelectItem: (id: string) => void;
  onResetFilters: () => void;
}

export const StartupTable: React.FC<StartupTableProps> = ({
  items,
  selectedId,
  onSelectItem,
  onResetFilters,
}) => {
  return (
    <div className="flex-1 flex flex-col min-h-0 rounded-xl lunar-glass-card overflow-hidden">
      {/* Table Column Header Matching ProcessTable */}
      <div className="grid grid-cols-12 items-center gap-3 px-4 py-2.5 bg-lunar-surface-2 border-b border-lunar-border text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted shrink-0">
        <div className="col-span-4 sm:col-span-3">Application</div>
        <div className="hidden sm:block sm:col-span-2">Source</div>
        <div className="col-span-2 sm:col-span-1 text-right">CPU</div>
        <div className="col-span-2 sm:col-span-2 lg:col-span-1 text-right">Memory</div>
        <div className="hidden lg:block lg:col-span-1 text-right">Disk</div>
        <div className="hidden lg:block lg:col-span-1 text-right">Delay</div>
        <div className="col-span-2 sm:col-span-2 lg:col-span-1 text-right">
          Impact
        </div>
        <div className="col-span-2 sm:col-span-2 lg:col-span-2 text-right">
          Status
        </div>
      </div>

      {/* Scrollable Rows */}
      <div className="flex-1 overflow-y-auto divide-y divide-lunar-border/40">
        {items.length === 0 ? (
          <div className="h-64 flex flex-col items-center justify-center text-center p-6">
            <SearchX className="w-8 h-8 text-lunar-muted mb-2.5 stroke-[1.5]" />
            <p className="text-sm font-medium text-lunar-text">
              No startup entries found.
            </p>
            <p className="text-xs text-lunar-muted mt-1 max-w-xs">
              No autostart applications match your current search query or active category filters.
            </p>
            <button
              type="button"
              onClick={onResetFilters}
              className="mt-4 px-3 py-1.5 rounded bg-lunar-elevated hover:bg-lunar-border text-xs text-lunar-white border border-lunar-border transition-colors cursor-pointer"
            >
              Reset Filters
            </button>
          </div>
        ) : (
          items.map((item) => (
            <StartupTableRow
              key={item.id}
              item={item}
              isSelected={selectedId === item.id}
              onSelect={onSelectItem}
            />
          ))
        )}
      </div>
    </div>
  );
};
