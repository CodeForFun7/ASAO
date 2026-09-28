import React, { useState, useMemo } from "react";
import type { ProcessResourceSample } from "../../types/process";
import { formatBytes, formatRate } from "../../services/tauri";

type ChartMetricTab = "cpu" | "memory" | "disk" | "network";

interface ProcessResourceChartProps {
  samples: ProcessResourceSample[];
}

export const ProcessResourceChart: React.FC<ProcessResourceChartProps> = ({
  samples,
}) => {
  const [activeTab, setActiveTab] = useState<ChartMetricTab>("cpu");

  const { points, maxLabel, currentLabel, avgLabel } = useMemo(() => {
    const paddedCount = 30;
    const rawValues = samples.map((s) => {
      switch (activeTab) {
        case "cpu":
          return s.cpuPercent;
        case "memory":
          return s.memoryBytes / (1024 * 1024); // MB
        case "disk":
          return s.diskBytesPerSec / 1024; // KB/s
        case "network":
          return s.networkBytesPerSec / 1024; // KB/s
      }
    });

    const latestVal = rawValues[rawValues.length - 1] ?? 0;
    const avgVal =
      rawValues.length > 0
        ? rawValues.reduce((a, b) => a + b, 0) / rawValues.length
        : 0;
    const peakVal = rawValues.length > 0 ? Math.max(...rawValues) : 0;

    // Pad from the left with initial sample or 0 so chart always spans 30 seconds
    const fillVal = rawValues[0] ?? 0;
    const padded =
      rawValues.length < paddedCount
        ? [
            ...Array(paddedCount - rawValues.length).fill(fillVal),
            ...rawValues,
          ]
        : rawValues.slice(rawValues.length - paddedCount);

    const upperBound =
      activeTab === "cpu"
        ? Math.max(15, Math.ceil(peakVal * 1.25), 25)
        : activeTab === "memory"
        ? Math.max(100, peakVal * 1.2)
        : Math.max(64, peakVal * 1.25);

    const width = 300;
    const height = 88;

    const coords = padded.map((val, idx) => {
      const x = (idx / (paddedCount - 1)) * width;
      const normalized = Math.min(1, Math.max(0, val / upperBound));
      const y = height - normalized * (height - 12) - 6;
      return { x, y };
    });

    const formatVal = (v: number) => {
      if (activeTab === "cpu") return `${v.toFixed(1)}%`;
      if (activeTab === "memory") return formatBytes(v * 1024 * 1024);
      return formatRate(v * 1024);
    };

    return {
      points: coords,
      maxLabel: formatVal(upperBound),
      currentLabel: formatVal(latestVal),
      avgLabel: formatVal(avgVal),
    };
  }, [samples, activeTab]);

  const polylineStr = points.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" ");
  const areaPathStr =
    points.length > 0
      ? `M 0,88 L ${points
          .map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`)
          .join(" L ")} L 300,88 Z`
      : "";

  return (
    <div className="rounded-md bg-lunar-bg border border-lunar-border p-3 space-y-2.5">
      {/* Metric Selector Tabs */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1 bg-lunar-surface p-0.5 rounded border border-lunar-border">
          {(
            [
              { id: "cpu", label: "CPU" },
              { id: "memory", label: "Memory" },
              { id: "disk", label: "Disk" },
              { id: "network", label: "Network" },
            ] as const
          ).map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              className={`px-2 py-0.5 rounded text-[10px] font-mono uppercase transition-colors cursor-pointer ${
                activeTab === tab.id
                  ? "bg-lunar-elevated text-lunar-white border border-lunar-border"
                  : "text-lunar-muted hover:text-lunar-text-sec"
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>

        <span className="text-[10px] font-mono text-lunar-muted">
          LAST 30S
        </span>
      </div>

      {/* SVG Graph */}
      <div className="relative h-[88px] w-full">
        <svg
          viewBox="0 0 300 88"
          preserveAspectRatio="none"
          className="w-full h-full overflow-visible"
        >
          <defs>
            <linearGradient id="lunarAreaFill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#E8EAED" stopOpacity="0.18" />
              <stop offset="100%" stopColor="#E8EAED" stopOpacity="0.01" />
            </linearGradient>
          </defs>

          {/* Horizontal Grid Lines */}
          <line
            x1="0"
            y1="14"
            x2="300"
            y2="14"
            stroke="#262C34"
            strokeDasharray="2,2"
            strokeWidth="1"
          />
          <line
            x1="0"
            y1="46"
            x2="300"
            y2="46"
            stroke="#262C34"
            strokeDasharray="2,2"
            strokeWidth="1"
          />
          <line
            x1="0"
            y1="82"
            x2="300"
            y2="82"
            stroke="#262C34"
            strokeWidth="1"
          />

          {/* Area Fill */}
          {areaPathStr && <path d={areaPathStr} fill="url(#lunarAreaFill)" />}

          {/* Line Path */}
          {polylineStr && (
            <polyline
              fill="none"
              stroke="#E8EAED"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
              points={polylineStr}
            />
          )}
        </svg>

        {/* Scale Label Overlay */}
        <div className="absolute top-0.5 right-1 text-[9px] font-mono text-lunar-muted pointer-events-none">
          MAX {maxLabel}
        </div>
      </div>

      {/* Time Axis & Live Readout */}
      <div className="flex items-center justify-between text-[10px] font-mono text-lunar-muted border-t border-lunar-border/60 pt-1.5">
        <div className="flex items-center gap-4">
          <span>-30s</span>
          <span>-20s</span>
          <span>-10s</span>
          <span className="text-lunar-text-sec">Now</span>
        </div>
        <div className="flex items-center gap-2.5">
          <span>
            AVG <strong className="text-lunar-text-sec font-normal">{avgLabel}</strong>
          </span>
          <span>
            NOW <strong className="text-lunar-white font-normal">{currentLabel}</strong>
          </span>
        </div>
      </div>
    </div>
  );
};
