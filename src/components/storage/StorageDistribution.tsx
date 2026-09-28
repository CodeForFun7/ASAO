import React from "react";
import {
  STORAGE_CATEGORY_META,
  type StorageCategory,
  type StorageCategorySlice,
} from "../../types/storage";
import { formatStorageBytes } from "../../services/storage";

interface StorageChartProps {
  slices: StorageCategorySlice[];
  totalUsedBytes: number;
}

export const StorageChart: React.FC<StorageChartProps> = ({
  slices,
  totalUsedBytes,
}) => {
  const size = 176;
  const strokeWidth = 15;
  const center = size / 2;
  const radius = center - strokeWidth;
  const circumference = 2 * Math.PI * radius;

  const nonZeroSlices = slices.filter((s) => s.bytes > 0);
  const sumBytes = nonZeroSlices.reduce((acc, s) => acc + s.bytes, 0);

  let cumulativeFraction = 0;

  return (
    <div className="relative w-[176px] h-[176px] mx-auto flex items-center justify-center select-none shrink-0">
      <svg
        width={size}
        height={size}
        viewBox={`0 0 ${size} ${size}`}
        className="-rotate-90 transform"
      >
        {/* Background track ring */}
        <circle
          cx={center}
          cy={center}
          r={radius}
          fill="transparent"
          stroke="#1E1E1E"
          strokeWidth={strokeWidth}
        />

        {/* Dynamic slices */}
        {sumBytes > 0 &&
          nonZeroSlices.map((slice) => {
            const fraction = slice.bytes / sumBytes;
            const dashLength = Math.max(0, fraction * circumference - 1.5);
            const dashOffset = -cumulativeFraction * circumference;
            cumulativeFraction += fraction;
            const meta =
              STORAGE_CATEGORY_META[slice.category] ??
              STORAGE_CATEGORY_META.UNKNOWN;

            return (
              <circle
                key={slice.category}
                cx={center}
                cy={center}
                r={radius}
                fill="transparent"
                stroke={meta.strokeColor}
                strokeWidth={strokeWidth}
                strokeDasharray={`${dashLength} ${circumference}`}
                strokeDashoffset={dashOffset}
                strokeLinecap="butt"
                className="transition-all duration-300"
              />
            );
          })}
      </svg>

      {/* Center Label */}
      <div className="absolute inset-0 flex flex-col items-center justify-center text-center pointer-events-none px-3">
        <span className="text-base font-semibold font-mono text-lunar-white tracking-tight leading-tight">
          {formatStorageBytes(totalUsedBytes > 0 ? totalUsedBytes : sumBytes)}
        </span>
        <span className="text-[10px] font-mono uppercase tracking-[0.16em] text-lunar-muted mt-0.5">
          USED
        </span>
      </div>
    </div>
  );
};

interface StorageDistributionProps {
  distribution: StorageCategorySlice[];
  totalUsedBytes: number;
  activeCategoryFilter: StorageCategory | "ALL";
  onSelectCategory: (cat: StorageCategory | "ALL") => void;
}

export const StorageDistribution: React.FC<StorageDistributionProps> = ({
  distribution,
  totalUsedBytes,
  activeCategoryFilter,
  onSelectCategory,
}) => {
  const sumBytes = distribution.reduce((acc, s) => acc + s.bytes, 0);
  const wholeBytes = Math.max(totalUsedBytes, sumBytes, 1);

  return (
    <div className="rounded-lg lunar-glass-card p-5 w-full">
      {/* Header */}
      <div className="flex items-center justify-between gap-2 mb-4">
        <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-muted">
          Storage Distribution
        </h2>
        {activeCategoryFilter !== "ALL" && (
          <button
            type="button"
            onClick={() => onSelectCategory("ALL")}
            className="px-2.5 py-1 rounded bg-lunar-elevated border border-lunar-border text-[11px] font-mono text-lunar-text-sec hover:text-lunar-white cursor-pointer"
          >
            Clear Category Filter
          </button>
        )}
      </div>

      {/* Stretched Horizontal Layout: Donut Chart (Left) + Category Line Bars (Right) */}
      <div className="flex flex-col md:flex-row items-center gap-6 lg:gap-8">
        {/* Left: Donut Chart */}
        <div className="shrink-0 flex flex-col items-center justify-center md:pl-2">
          <StorageChart
            slices={distribution}
            totalUsedBytes={wholeBytes}
          />
        </div>

        {/* Right: Stretched Category Breakdown with Proportional Line Charts */}
        <div className="flex-1 w-full min-w-0">
          {/* Individual Category Rows with Line Chart */}
          <div className="divide-y divide-lunar-border/40">
            {distribution.map((slice) => {
              const meta =
                STORAGE_CATEGORY_META[slice.category] ??
                STORAGE_CATEGORY_META.UNKNOWN;
              const isSelected = activeCategoryFilter === slice.category;
              const isEmpty = slice.bytes <= 0;
              const sharePercent =
                wholeBytes > 0
                  ? Math.min(100, (slice.bytes / wholeBytes) * 100)
                  : 0;
              const barWidthPercent =
                slice.bytes > 0 ? Math.max(1.5, sharePercent) : 0;

              return (
                <button
                  key={slice.category}
                  type="button"
                  onClick={() =>
                    onSelectCategory(isSelected ? "ALL" : slice.category)
                  }
                  className={`w-full py-2 px-2.5 flex items-center gap-4 text-left rounded transition-colors cursor-pointer ${
                    isSelected
                      ? "bg-lunar-elevated text-lunar-white"
                      : "hover:bg-lunar-surface-2/80"
                  } ${isEmpty ? "opacity-50" : ""}`}
                >
                  {/* Category Dot + Label */}
                  <div className="flex items-center gap-2.5 w-36 sm:w-40 shrink-0 min-w-0">
                    <span
                      className={`w-2.5 h-2.5 rounded-xs shrink-0 ${meta.dotColor}`}
                    />
                    <span className="text-xs font-medium text-lunar-text truncate">
                      {slice.label}
                    </span>
                  </div>

                  {/* Proportional Line Chart showing share out of whole */}
                  <div className="flex-1 min-w-[100px] flex items-center">
                    <div className="w-full h-2 bg-lunar-bg border border-lunar-border/80 rounded-full overflow-hidden p-[1px]">
                      <div
                        className="h-full rounded-full transition-all duration-300"
                        style={{
                          width: `${barWidthPercent}%`,
                          backgroundColor: meta.strokeColor,
                        }}
                      />
                    </div>
                  </div>

                  {/* Percentage + Bytes */}
                  <div className="flex items-center gap-3 shrink-0 font-mono text-xs">
                    <span className="text-[11px] text-lunar-muted w-12 text-right">
                      {slice.bytes > 0
                        ? `${sharePercent.toFixed(1)}%`
                        : "0.0%"}
                    </span>
                    <span className="text-lunar-white font-semibold w-20 text-right">
                      {formatStorageBytes(slice.bytes)}
                    </span>
                  </div>
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
};
