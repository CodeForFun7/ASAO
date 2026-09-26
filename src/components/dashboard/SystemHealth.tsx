import React, { useMemo } from "react";
import type {
  SystemMetrics,
  SystemTelemetryPoint,
} from "../../types/process";
import { formatBytes } from "../../services/tauri";

interface SystemHealthProps {
  metrics: SystemMetrics;
  history: SystemTelemetryPoint[];
  onSelectCpu: () => void;
  onSelectMemory: () => void;
  onSelectGpu: () => void;
}

interface ActivityColumnChartProps {
  title: string;
  currentValueLabel: string;
  currentPercent: number;
  values: number[];
  strokeColor: string;
  badgeTextColor?: string;
  gradientId: string;
  primaryStatLabel: string;
  primaryStatValue: string;
  secondaryStatLabel: string;
  secondaryStatValue: string;
  onClick: () => void;
}

function buildSmoothCurvePath(coords: { x: number; y: number }[]): string {
  if (coords.length === 0) return "";
  if (coords.length === 1) return `M ${coords[0].x.toFixed(1)},${coords[0].y.toFixed(1)}`;

  let d = `M ${coords[0].x.toFixed(1)},${coords[0].y.toFixed(1)}`;
  for (let i = 0; i < coords.length - 1; i++) {
    const p0 = coords[i];
    const p1 = coords[i + 1];
    const midX = (p0.x + p1.x) / 2;
    d += ` C ${midX.toFixed(1)},${p0.y.toFixed(1)} ${midX.toFixed(1)},${p1.y.toFixed(1)} ${p1.x.toFixed(1)},${p1.y.toFixed(1)}`;
  }
  return d;
}

const ActivityColumnChart: React.FC<ActivityColumnChartProps> = ({
  title,
  currentValueLabel,
  currentPercent,
  values,
  strokeColor,
  badgeTextColor = "#0B0D10",
  gradientId,
  primaryStatLabel,
  primaryStatValue,
  secondaryStatLabel,
  secondaryStatValue,
  onClick,
}) => {
  const { curvePath, areaPath, highlightPoint } = useMemo(() => {
    const targetCount = 18;
    const fillVal = values[0] ?? currentPercent;
    const sampled =
      values.length < targetCount
        ? [...Array(targetCount - values.length).fill(fillVal), ...values]
        : values.slice(values.length - targetCount);

    const width = 240;
    const height = 104;
    const topPad = 26;
    const bottomPad = 8;
    const usableHeight = height - topPad - bottomPad;

    const coords = sampled.map((v, i) => {
      const x = (i / (targetCount - 1)) * width;
      const clamped = Math.min(100, Math.max(0, v));
      const y = topPad + usableHeight - (clamped / 100) * usableHeight;
      return { x, y, val: clamped };
    });

    // Pick the peak point in the central window (indices 5..14) so the callout pill sits nicely like in the reference image
    let peakIdx = Math.floor(targetCount * 0.6);
    let maxVal = -1;
    for (let i = 4; i < targetCount - 3; i++) {
      if (coords[i].val >= maxVal) {
        maxVal = coords[i].val;
        peakIdx = i;
      }
    }

    const line = buildSmoothCurvePath(coords);
    const area = `${line} L ${width},${height - bottomPad} L 0,${height - bottomPad} Z`;

    return {
      curvePath: line,
      areaPath: area,
      highlightPoint: coords[peakIdx] ?? coords[coords.length - 1],
    };
  }, [values, currentPercent]);

  const badgeText = `${currentPercent.toFixed(0)}%`;

  return (
    <div
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      className="group flex flex-col justify-between rounded-lg p-2.5 -m-2.5 hover:bg-lunar-elevated/30 transition-colors cursor-pointer"
    >
      {/* Top: Smooth Curve Chart with Floating Callout Badge & Dashed Drop Line */}
      <div className="relative h-[108px] w-full mb-3">
        <svg
          viewBox="0 0 240 104"
          preserveAspectRatio="none"
          className="w-full h-full overflow-visible"
        >
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={strokeColor} stopOpacity="0.24" />
              <stop offset="100%" stopColor={strokeColor} stopOpacity="0.0" />
            </linearGradient>
          </defs>

          {/* Soft Area Fill */}
          <path d={areaPath} fill={`url(#${gradientId})`} />

          {/* Smooth Curve Line */}
          <path
            d={curvePath}
            fill="none"
            stroke={strokeColor}
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />

          {/* Highlighted Point Vertical Dashed Line + Callout Badge */}
          {highlightPoint && (
            <g>
              <line
                x1={highlightPoint.x}
                y1={highlightPoint.y + 4}
                x2={highlightPoint.x}
                y2={96}
                stroke={strokeColor}
                strokeWidth="1.25"
                strokeDasharray="2,4"
                strokeOpacity="0.75"
              />
              <circle
                cx={highlightPoint.x}
                cy={highlightPoint.y}
                r="4"
                fill={strokeColor}
                stroke="#0B0D10"
                strokeWidth="1.5"
              />
              <rect
                x={highlightPoint.x - 19}
                y={Math.max(1, highlightPoint.y - 22)}
                width="38"
                height="16"
                rx="8"
                fill={strokeColor}
              />
              <text
                x={highlightPoint.x}
                y={Math.max(1, highlightPoint.y - 22) + 11}
                textAnchor="middle"
                fill={badgeTextColor}
                fontSize="9.5"
                fontWeight="700"
                fontFamily="IBM Plex Mono, monospace"
              >
                {badgeText}
              </text>
            </g>
          )}
        </svg>
      </div>

      {/* Middle: Metric Title & Big Value (matching Steps / Calories / Activity time in ref) */}
      <div className="mb-3">
        <div className="text-xs font-medium text-lunar-text-sec group-hover:text-lunar-white transition-colors">
          {title}
        </div>
        <div className="text-2xl font-semibold text-lunar-white font-mono tracking-tight mt-1">
          {currentValueLabel}
        </div>
      </div>

      {/* Bottom: Two Stat Rows (matching Goal / Average in ref) */}
      <div className="space-y-1.5 pt-2 border-t border-lunar-border/50 text-[11px] font-mono">
        <div className="flex items-center justify-between text-lunar-muted">
          <span>{primaryStatLabel}</span>
          <span className="text-lunar-text-sec">{primaryStatValue}</span>
        </div>
        <div className="flex items-center justify-between text-lunar-muted">
          <span>{secondaryStatLabel}</span>
          <span className="text-lunar-text-sec">{secondaryStatValue}</span>
        </div>
      </div>
    </div>
  );
};

export const SystemHealth: React.FC<SystemHealthProps> = ({
  metrics,
  history,
  onSelectCpu,
  onSelectMemory,
  onSelectGpu,
}) => {
  const cpuSeries = useMemo(() => history.map((h) => h.cpuPercent), [history]);
  const memSeries = useMemo(
    () => history.map((h) => h.memoryPercent),
    [history]
  );
  const gpuSeries = useMemo(() => history.map((h) => h.gpuPercent), [history]);

  const cpuStats = useMemo(() => {
    const arr = cpuSeries.length ? cpuSeries : [metrics.cpuUsagePercent];
    const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
    const peak = Math.max(...arr, metrics.cpuUsagePercent);
    return { avg, peak };
  }, [cpuSeries, metrics.cpuUsagePercent]);

  const memStats = useMemo(() => {
    const arr = memSeries.length ? memSeries : [metrics.memoryUsagePercent];
    const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
    return { avg };
  }, [memSeries, metrics.memoryUsagePercent]);

  const gpuStats = useMemo(() => {
    const cur = metrics.gpuUsagePercent ?? 0;
    const arr = gpuSeries.length ? gpuSeries : [cur];
    const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
    const peak = Math.max(...arr, cur);
    return { avg, peak };
  }, [gpuSeries, metrics.gpuUsagePercent]);

  return (
    <section className="flex-1 rounded-xl lunar-glass-card p-5 flex flex-col justify-between">
      {/* Header Row (matching "Physical Activity" + "Today v" in ref) */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2.5">
          <h2 className="text-sm font-semibold text-lunar-white tracking-tight">
            System Activity
          </h2>
          <span className="w-1.5 h-1.5 rounded-full bg-lunar-healthy animate-pulse" />
        </div>
        <span className="text-xs font-mono text-lunar-text-sec px-2.5 py-1 rounded-md bg-lunar-bg/60 border border-lunar-border">
          Live · 30s
        </span>
      </div>

      {/* 3 Side-by-Side Columns (CPU, GPU, RAM) */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6 md:divide-x md:divide-lunar-border/40">
        {/* Column 1: CPU */}
        <div className="md:pr-2">
          <ActivityColumnChart
            title="CPU Usage"
            currentValueLabel={`${metrics.cpuUsagePercent.toFixed(0)}%`}
            currentPercent={metrics.cpuUsagePercent}
            values={cpuSeries}
            strokeColor="#9BAE9F"
            badgeTextColor="#0B0D10"
            gradientId="gradCpuActivity"
            primaryStatLabel="Peak"
            primaryStatValue={`${cpuStats.peak.toFixed(0)}%`}
            secondaryStatLabel="Average"
            secondaryStatValue={`${cpuStats.avg.toFixed(1)}%`}
            onClick={onSelectCpu}
          />
        </div>

        {/* Column 2: GPU */}
        <div className="md:px-4">
          <ActivityColumnChart
            title="GPU Usage"
            currentValueLabel={`${(metrics.gpuUsagePercent ?? 0).toFixed(0)}%`}
            currentPercent={metrics.gpuUsagePercent ?? 0}
            values={gpuSeries}
            strokeColor="#C9A66B"
            badgeTextColor="#0B0D10"
            gradientId="gradGpuActivity"
            primaryStatLabel="Peak"
            primaryStatValue={`${gpuStats.peak.toFixed(0)}%`}
            secondaryStatLabel="Average"
            secondaryStatValue={`${gpuStats.avg.toFixed(1)}%`}
            onClick={onSelectGpu}
          />
        </div>

        {/* Column 3: RAM */}
        <div className="md:pl-4">
          <ActivityColumnChart
            title="RAM Usage"
            currentValueLabel={formatBytes(metrics.memoryUsedBytes)}
            currentPercent={metrics.memoryUsagePercent}
            values={memSeries}
            strokeColor="#A6A1B8"
            badgeTextColor="#0B0D10"
            gradientId="gradRamActivity"
            primaryStatLabel="Capacity"
            primaryStatValue={formatBytes(metrics.memoryTotalBytes)}
            secondaryStatLabel="Average"
            secondaryStatValue={`${memStats.avg.toFixed(1)}%`}
            onClick={onSelectMemory}
          />
        </div>
      </div>
    </section>
  );
};

