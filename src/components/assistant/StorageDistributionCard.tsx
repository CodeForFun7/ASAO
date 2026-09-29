import React from "react";
import { HardDrive, ArrowUpRight } from "lucide-react";
import type { DiagnosticVisualData } from "../../types/agent";
import { useProcessStore } from "../../stores/process-store";

interface StorageDistributionCardProps {
  data: DiagnosticVisualData;
}

const CATEGORY_SHADES: Record<string, string> = {
  SYSTEM: "bg-white",
  APPLICATION: "bg-zinc-300",
  USER: "bg-zinc-400",
  CACHE: "bg-zinc-500",
  TEMPORARY: "bg-zinc-600",
  UNKNOWN: "bg-zinc-700",
};

export const StorageDistributionCard: React.FC<StorageDistributionCardProps> = ({ data }) => {
  const storage = data.storage;
  const setRoute = useProcessStore((s) => s.setRoute);

  if (!storage) return null;

  const primaryDrive = storage.drives?.[0];
  const categories = storage.categories || [];
  const largestFiles = storage.largestFiles || [];

  return (
    <div className="rounded-2xl bg-white/[0.03] border border-white/[0.08] p-4 my-3 text-xs">
      <div className="flex items-center justify-between mb-3.5">
        <div className="flex items-center gap-2 font-medium text-white text-[12.5px]">
          <HardDrive className="w-4 h-4 text-white" />
          <span>Storage Utilization ({primaryDrive?.drive || "Drive C:"})</span>
        </div>
        {primaryDrive && (
          <span className="font-mono text-[11px] text-zinc-400">
            {primaryDrive.used} used / {primaryDrive.total}
          </span>
        )}
      </div>

      {/* Main Drive Capacity Bar */}
      {primaryDrive && (
        <div className="mb-3.5">
          <div className="flex items-center justify-between text-[11px] text-zinc-400 mb-1.5">
            <span>Volume Capacity</span>
            <span className="font-mono font-medium text-white">
              {primaryDrive.usagePercentage.toFixed(0)}% Used ({primaryDrive.free} free)
            </span>
          </div>
          <div className="h-2 w-full rounded-full bg-white/[0.08] overflow-hidden">
            <div
              style={{ width: `${Math.min(100, primaryDrive.usagePercentage)}%` }}
              className="h-full bg-white transition-all duration-500"
            />
          </div>
        </div>
      )}

      {/* Category Segmented Distribution Bar (Monochrome shades of white) */}
      {categories.length > 0 && (
        <div className="space-y-2 mb-3.5">
          <div className="text-[10px] text-zinc-400 uppercase tracking-wider font-semibold">
            Composition by Category
          </div>

          <div className="h-2.5 w-full rounded-full bg-white/[0.08] overflow-hidden flex">
            {categories.map((cat) => {
              const bg = CATEGORY_SHADES[cat.category] || "bg-zinc-600";
              return (
                <div
                  key={cat.category}
                  style={{ width: `${Math.max(1, cat.percentage)}%` }}
                  className={`h-full ${bg} transition-all duration-500`}
                  title={`${cat.label}: ${cat.sizeFormatted} (${cat.percentage}%)`}
                />
              );
            })}
          </div>

          {/* Category Legend Chips */}
          <div className="flex flex-wrap gap-2.5 pt-1 text-[10.5px]">
            {categories.map((cat) => {
              const bg = CATEGORY_SHADES[cat.category] || "bg-zinc-600";
              return (
                <div key={cat.category} className="flex items-center gap-1.5 text-zinc-400">
                  <span className={`w-2 h-2 rounded-full ${bg} shrink-0 inline-block`} />
                  <span>{cat.label}:</span>
                  <span className="font-mono text-white font-medium">
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
        <div className="pt-2.5 border-t border-white/[0.06]">
          <div className="text-[10px] text-zinc-400 uppercase tracking-wider font-semibold mb-2">
            Top Large Files
          </div>
          <div className="space-y-1.5">
            {largestFiles.slice(0, 3).map((f) => (
              <div
                key={f.path}
                className="flex items-center justify-between text-[11px] py-1 px-2.5 rounded-lg bg-white/[0.02] border border-white/[0.06]"
              >
                <span className="truncate max-w-[260px] text-zinc-300 font-mono text-[10.5px]">
                  {f.name}
                </span>
                <span className="font-mono font-medium text-white text-[11px] shrink-0 pl-2">
                  {f.sizeFormatted}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Footer jump button */}
      <div className="mt-3.5 pt-3 border-t border-white/[0.06] flex justify-end">
        <button
          type="button"
          onClick={() => setRoute("storage")}
          className="text-white hover:text-zinc-300 flex items-center gap-1 text-[11px] font-medium transition-colors cursor-pointer"
        >
          <span>Storage Explorer</span>
          <ArrowUpRight className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
};
