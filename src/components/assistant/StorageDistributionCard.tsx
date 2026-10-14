import React from "react";
import { HardDrive, ArrowUpRight } from "lucide-react";
import type { DiagnosticVisualData } from "../../types/agent";
import { STORAGE_CATEGORY_META, type StorageCategory } from "../../types/storage";
import { useProcessStore } from "../../stores/process-store";

interface StorageDistributionCardProps {
  data: DiagnosticVisualData;
}

export const StorageDistributionCard: React.FC<StorageDistributionCardProps> = ({ data }) => {
  const storage = data.storage;
  const setRoute = useProcessStore((s) => s.setRoute);

  if (!storage) return null;

  const primaryDrive = storage.drives?.[0];
  const categories = storage.categories || [];
  const largestFiles = storage.largestFiles || [];

  return (
    <div className="rounded-2xl bg-lunar-surface/70 border border-lunar-border p-4 my-3 text-xs">
      <div className="flex items-center justify-between mb-3.5">
        <div className="flex items-center gap-2 font-medium text-lunar-text text-[12.5px]">
          <HardDrive className="w-4 h-4 text-lunar-white" />
          <span>Storage Utilization ({primaryDrive?.drive || "Drive C:"})</span>
        </div>
        {primaryDrive && (
          <span className="font-mono text-[11px] text-lunar-subtext">
            {primaryDrive.used} used / {primaryDrive.total}
          </span>
        )}
      </div>

      {/* Main Drive Capacity Bar */}
      {primaryDrive && (
        <div className="mb-3.5">
          <div className="flex items-center justify-between text-[11px] text-lunar-subtext mb-1.5">
            <span>Volume Capacity</span>
            <span className="font-mono font-medium text-lunar-text">
              {primaryDrive.usagePercentage.toFixed(0)}% Used ({primaryDrive.free} free)
            </span>
          </div>
          <div className="h-2 w-full rounded-full bg-lunar-border/30 overflow-hidden">
            <div
              style={{ width: `${Math.min(100, primaryDrive.usagePercentage)}%` }}
              className="h-full bg-lunar-white transition-all duration-500"
            />
          </div>
        </div>
      )}

      {/* Category Segmented Distribution Bar */}
      {categories.length > 0 && (
        <div className="space-y-2 mb-3.5">
          <div className="text-[10px] text-lunar-subtext uppercase tracking-wider font-semibold">
            Composition by Category
          </div>

          <div className="h-2.5 w-full rounded-full bg-lunar-border/30 overflow-hidden flex">
            {categories.map((cat) => {
              const meta = STORAGE_CATEGORY_META[cat.category as StorageCategory];
              const dotClass = meta?.dotColor || "bg-lunar-muted";
              return (
                <div
                  key={cat.category}
                  style={{ width: `${Math.max(1, cat.percentage)}%` }}
                  className={`h-full ${dotClass} transition-all duration-500`}
                  title={`${cat.label}: ${cat.sizeFormatted} (${cat.percentage}%)`}
                />
              );
            })}
          </div>

          {/* Category Legend Chips */}
          <div className="flex flex-wrap gap-2.5 pt-1 text-[10.5px]">
            {categories.map((cat) => {
              const meta = STORAGE_CATEGORY_META[cat.category as StorageCategory];
              const dotClass = meta?.dotColor || "bg-lunar-muted";
              return (
                <div key={cat.category} className="flex items-center gap-1.5 text-lunar-subtext">
                  <span className={`w-2 h-2 rounded-full ${dotClass} shrink-0 inline-block`} />
                  <span>{cat.label}:</span>
                  <span className="font-mono text-lunar-text font-medium">
                    {cat.sizeFormatted}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Largest Files sample */}
      {largestFiles.length > 0 && (
        <div className="pt-2.5 border-t border-lunar-border/50">
          <div className="text-[10px] text-lunar-subtext uppercase tracking-wider font-semibold mb-2">
            Top Large Files
          </div>
          <div className="space-y-1.5">
            {largestFiles.slice(0, 3).map((f) => (
              <div
                key={f.path}
                className="flex items-center justify-between text-[11px] py-1 px-2.5 rounded-lg bg-lunar-surface/40 border border-lunar-border/50"
              >
                <span className="truncate max-w-[260px] text-lunar-text font-mono text-[10.5px]">
                  {f.name}
                </span>
                <span className="font-mono font-medium text-lunar-white text-[11px] shrink-0 pl-2">
                  {f.sizeFormatted}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Footer jump button */}
      <div className="mt-3.5 pt-3 border-t border-lunar-border/50 flex justify-end">
        <button
          type="button"
          onClick={() => setRoute("storage")}
          className="text-lunar-text hover:text-lunar-subtext flex items-center gap-1 text-[11px] font-medium transition-colors cursor-pointer"
        >
          <span>Storage Explorer</span>
          <ArrowUpRight className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
};
