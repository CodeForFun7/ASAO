import React, { useLayoutEffect, useMemo, useRef, useState } from "react";
import type {
  SystemMetrics,
  SystemTelemetryPoint,
} from "../../types/process";

interface SystemHealthProps {
  metrics: SystemMetrics;
  history: SystemTelemetryPoint[];
  onSelectCpu: () => void;
  onSelectMemory: () => void;
}

interface ActivityColumnChartProps {
  title: string;
  badgeText: string;
  currentPercent: number;
  values: number[];
  strokeColor: string;
  badgeTextColor?: string;
  gradientId: string;
  peakValue: string;
  averageValue: string;
  onClick: () => void;
}

function buildSmoothCurvePath(coords: { x: number; y: number }[]): string {
  if (coords.length === 0) return "";
  if (coords.length === 1)
    return `M ${coords[0].x.toFixed(1)},${coords[0].y.toFixed(1)}`;

  let d = `M ${coords[0].x.toFixed(1)},${coords[0].y.toFixed(1)}`;
  for (let i = 0; i < coords.length - 1; i++) {
    const p0 = coords[i];
    const p1 = coords[i + 1];
    const midX = (p0.x + p1.x) / 2;
    d += ` C ${midX.toFixed(1)},${p0.y.toFixed(1)} ${midX.toFixed(
      1
    )},${p1.y.toFixed(1)} ${p1.x.toFixed(1)},${p1.y.toFixed(1)}`;
  }
  return d;
}

/** Tracks an element's pixel size so the SVG can be drawn 1:1 instead of being stretched. */
function useElementSize<T extends HTMLElement>() {
  const ref = useRef<T | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => {
      const { width, height } = el.getBoundingClientRect();
      setSize((prev) =>
        Math.abs(prev.width - width) < 1 && Math.abs(prev.height - height) < 1
          ? prev
          : { width, height }
      );
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return [ref, size] as const;
}

const ActivityColumnChart: React.FC<ActivityColumnChartProps> = ({
  title,
  badgeText,
  currentPercent,
  values,
  strokeColor,
  badgeTextColor = "var(--color-lunar-bg)",
  gradientId,
  peakValue,
  averageValue,
  onClick,
}) => {
  const [chartRef, { width, height }] = useElementSize<HTMLDivElement>();

  const { curvePath, areaPath, highlightPoint } = useMemo(() => {
    if (width < 8 || height < 8) {
      return { curvePath: "", areaPath: "", highlightPoint: null };
    }

    const targetCount = 18;
    const fillVal = values[0] ?? currentPercent;
    const sampled =
      values.length < targetCount
        ? [...Array(targetCount - values.length).fill(fillVal), ...values]
        : values.slice(values.length - targetCount);

    const topPad = 26;
    const bottomPad = 6;
    const usableHeight = Math.max(8, height - topPad - bottomPad);

    const coords = sampled.map((v, i) => {
      const x = (i / (targetCount - 1)) * width;
      const clamped = Math.min(100, Math.max(0, v));
      const y = topPad + usableHeight - (clamped / 100) * usableHeight;
      return { x, y, val: clamped };
    });

    let peakIdx = Math.floor(targetCount * 0.6);
    let maxVal = -1;
    for (let i = 4; i < targetCount - 3; i++) {
      if (coords[i].val >= maxVal) {
        maxVal = coords[i].val;
        peakIdx = i;
      }
    }

    const line = buildSmoothCurvePath(coords);
    const area = `${line} L ${width},${height - bottomPad} L 0,${
      height - bottomPad
    } Z`;

    return {
      curvePath: line,
      areaPath: area,
      highlightPoint: coords[peakIdx] ?? coords[coords.length - 1],
    };
  }, [values, currentPercent, width, height]);

  const badgeWidth = Math.max(38, badgeText.length * 6.2 + 12);

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
      className="group flex flex-col min-h-0 min-w-0 h-full rounded-lg hover:bg-lunar-elevated/30 transition-colors cursor-pointer"
    >
      <div className="flex items-center justify-between gap-2 mb-1 shrink-0">
        <span className="text-xs font-semibold text-lunar-text tracking-wide group-hover:text-lunar-white transition-colors">
          {title}
        </span>
        <span className="text-[11px] font-mono text-lunar-muted truncate">
          Peak {peakValue} · Avg {averageValue}
        </span>
      </div>

      {/* Chart fills the remaining height; the SVG is sized to the measured box */}
      <div ref={chartRef} className="relative flex-1 min-h-[64px] w-full">
        {width > 0 && height > 0 && (
          <svg
            width={width}
            height={height}
            viewBox={`0 0 ${width} ${height}`}
            className="absolute inset-0 overflow-visible"
          >
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor={strokeColor} stopOpacity="0.24" />
                <stop offset="100%" stopColor={strokeColor} stopOpacity="0.0" />
              </linearGradient>
            </defs>

            <path d={areaPath} fill={`url(#${gradientId})`} />

            <path
              d={curvePath}
              fill="none"
              stroke={strokeColor}
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />

            {highlightPoint && (
              <g>
                <line
                  x1={highlightPoint.x}
                  y1={highlightPoint.y + 4}
                  x2={highlightPoint.x}
                  y2={height - 6}
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
                  stroke="var(--color-lunar-surface)"
                  strokeWidth="1.5"
                />
                <rect
                  x={Math.min(
                    Math.max(0, highlightPoint.x - badgeWidth / 2),
                    width - badgeWidth
                  )}
                  y={Math.max(1, highlightPoint.y - 22)}
                  width={badgeWidth}
                  height="16"
                  rx="8"
                  fill={strokeColor}
                />
                <text
                  x={Math.min(
                    Math.max(badgeWidth / 2, highlightPoint.x),
                    width - badgeWidth / 2
                  )}
                  y={Math.max(1, highlightPoint.y - 22) + 11}
                  textAnchor="middle"
                  fill={badgeTextColor}
                  fontSize="9"
                  fontWeight="700"
                  fontFamily="IBM Plex Mono, monospace"
                >
                  {badgeText}
                </text>
              </g>
            )}
          </svg>
        )}
      </div>
    </div>
  );
};

export const SystemHealth: React.FC<SystemHealthProps> = ({
  metrics,
  history,
  onSelectCpu,
  onSelectMemory,
}) => {
  const cpuSeries = useMemo(() => history.map((h) => h.cpuPercent), [history]);
  const memSeries = useMemo(
    () => history.map((h) => h.memoryPercent),
    [history]
  );

  const cpuStats = useMemo(() => {
    const arr = cpuSeries.length ? cpuSeries : [metrics.cpuUsagePercent];
    const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
    const peak = Math.max(...arr, metrics.cpuUsagePercent);
    return { avg, peak };
  }, [cpuSeries, metrics.cpuUsagePercent]);

  const memStats = useMemo(() => {
    const arr = memSeries.length ? memSeries : [metrics.memoryUsagePercent];
    const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
    const peak = Math.max(...arr, metrics.memoryUsagePercent);
    return { avg, peak };
  }, [memSeries, metrics.memoryUsagePercent]);

  return (
    <section className="h-full min-h-0 rounded-xl lunar-glass-card p-4 grid grid-cols-1 sm:grid-cols-2 gap-4 sm:gap-6 sm:divide-x sm:divide-lunar-border/60">
      <div className="min-h-[120px] sm:min-h-0 sm:pr-3 min-w-0">
        <ActivityColumnChart
          title="CPU"
          badgeText={`${metrics.cpuUsagePercent.toFixed(0)}%`}
          currentPercent={metrics.cpuUsagePercent}
          values={cpuSeries}
          strokeColor="var(--color-lunar-white)"
          badgeTextColor="var(--color-lunar-bg)"
          gradientId="gradCpuActivity"
          peakValue={`${cpuStats.peak.toFixed(0)}%`}
          averageValue={`${cpuStats.avg.toFixed(1)}%`}
          onClick={onSelectCpu}
        />
      </div>

      <div className="min-h-[120px] sm:min-h-0 sm:pl-6 min-w-0">
        <ActivityColumnChart
          title="RAM"
          badgeText={`${metrics.memoryUsagePercent.toFixed(0)}%`}
          currentPercent={metrics.memoryUsagePercent}
          values={memSeries}
          strokeColor="var(--color-lunar-white)"
          badgeTextColor="var(--color-lunar-bg)"
          gradientId="gradRamActivity"
          peakValue={`${memStats.peak.toFixed(0)}%`}
          averageValue={`${memStats.avg.toFixed(1)}%`}
          onClick={onSelectMemory}
        />
      </div>
    </section>
  );
};
