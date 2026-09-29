import React from "react";
import { SearchX } from "lucide-react";
import type { ProcessInfo } from "../../types/process";
import { ProcessRow } from "./ProcessRow";

interface ProcessTableProps {
  processes: ProcessInfo[];
  selectedPid: number | null;
  onSelectProcess: (pid: number) => void;
  onResetFilters: () => void;
}

export const ProcessTable: React.FC<ProcessTableProps> = ({
  processes,
  selectedPid,
  onSelectProcess,
  onResetFilters,
}) => {
  return (
    <div className="flex-1 flex flex-col min-h-0 rounded-xl lunar-glass-card overflow-hidden">
      {/* Table Column Header */}
      <div className="grid grid-cols-12 items-center gap-3 px-4 py-2.5 bg-lunar-surface-2 border-b border-lunar-border text-[10px] font-mono uppercase tracking-[0.14em] text-lunar-muted shrink-0">
        <div className="col-span-4 sm:col-span-3">Process</div>
        <div className="hidden sm:block sm:col-span-2 text-center">Category</div>
        <div className="col-span-2 sm:col-span-1 text-right">CPU</div>
        <div className="col-span-2 sm:col-span-2 text-right">Memory</div>
        <div className="hidden lg:block lg:col-span-1 text-right">Disk</div>
        <div className="hidden lg:block lg:col-span-1 text-right">Network</div>
        <div className="col-span-4 sm:col-span-4 lg:col-span-2 text-center">
          Resource
        </div>
      </div>

      {/* Scrollable Rows */}
      <div className="flex-1 overflow-y-auto divide-y divide-lunar-border/40">
        {processes.length === 0 ? (
          <div className="h-64 flex flex-col items-center justify-center text-center p-6">
            <SearchX className="w-8 h-8 text-lunar-muted mb-2.5 stroke-[1.5]" />
            <p className="text-sm font-medium text-lunar-text">
              No processes found.
            </p>
            <p className="text-xs text-lunar-muted mt-1 max-w-xs">
              No active processes match your current search query or filter
              criteria.
            </p>
            <button
              type="button"
              onClick={onResetFilters}
              className="mt-4 px-3 py-1.5 rounded bg-lunar-elevated hover:bg-lunar-border text-xs text-lunar-white border border-lunar-border transition-colors cursor-pointer"
            >
              Reset Active Filters
            </button>
          </div>
        ) : (
          processes.map((proc) => (
            <ProcessRow
              key={proc.pid}
              process={proc}
              isSelected={selectedPid === proc.pid}
              onSelect={onSelectProcess}
            />
          ))
        )}
      </div>
    </div>
  );
};
