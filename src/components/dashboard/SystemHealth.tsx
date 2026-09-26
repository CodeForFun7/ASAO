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

interface TelemetryMiniChartProps {
  title: string;
  subtitle: string;
  currentPercent: number;
  deltaPercent: number;
  values: number[];
  strokeColor: string;
  gradientId: string;
  onClick: () => void;
}

const TelemetryMiniChart: React.FC<TelemetryMiniChartProps> = ({
  title,
  subtitle,
  currentPercent,
  deltaPercent,
  values,
  strokeColor,
  gradientId,
  onClick,
}) => {
  const { points, polylineStr, areaPathStr, avgVal, peakVal } = useMemo(() => {
    const targetCount = 30;
    const fillVal = values[0] ?? currentPercent;
    const padded =
      values.length < targetCount
        ? [...Array(targetCount - values.length).fill(fillVal), ...values]
        : values.slice(values.length - targetCount);

    const avg =
      padded.reduce((acc, v) => acc + v, 0) / Math.max(1, padded.length);
    const peak = Math.max(...padded, currentPercent);

    const width = 320;
    const height = 104;

    const coords = padded.map((v, i) => {
      const x = (i / (targetCount - 1)) * width;
      const clamped = Math.min(100, Math.max(0, v));
      const y = height - (clamped / 100) * (height - 16) - 6;
      return { x, y };
    });

    const line = coords
      .map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`)
      .join(" ");
    const area = `M 0,${height} L ${coords
      .map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`)
      .join(" L ")} L ${width},${height} Z`;

    return {
      points: coords,
      polylineStr: line,
      areaPathStr: area,
      avgVal: avg,
      peakVal: peak,
    };
  }, [values, currentPercent]);

  const lastPoint = points[points.length - 1];

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
      className="group rounded-lg bg-lunar-bg/70 hover:bg-lunar-surface-2 border border-lunar-border hover:border-lunar-text-sec/40 p-4 flex flex-col justify-between transition-colors cursor-pointer"
    >
      {/* Header */}
      <div className="flex items-start justify-between gap-2 mb-3">
        <div>
          <div className="flex items-center gap-2">
            <span
              className="w-2 h-2 rounded-full"
              style={{ backgroundColor: strokeColor }}
            />
            <span className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-white font-semibold">
              {title}
            </span>
          </div>
          <div className="text-[11px] font-mono text-lunar-muted mt-0.5">
            {subtitle}
          </div>
        </div>

        <div className="text-right font-mono">
          <div className="text-xl font-semibold text-lunar-white leading-none">
            {currentPercent.toFixed(0)}%
          </div>
          <div className="text-[10px] text-lunar-text-sec mt-1">
            {deltaPercent >= 0 ? `↑ ${deltaPercent.toFixed(1)}%` : `↓ ${Math.abs(deltaPercent).toFixed(1)}%`}
          </div>
        </div>
      </div>

      {/* SVG Area Chart */}
      <div className="relative h-[104px] w-full">
        <svg
          viewBox="0 0 320 104"
          preserveAspectRatio="none"
          className="w-full h-full overflow-visible"
        >
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={strokeColor} stopOpacity="0.26" />
              <stop offset="100%" stopColor={strokeColor} stopOpacity="0.01" />
            </linearGradient>
          </defs>

          {/* Horizontal Reference Grid Lines (75%, 50%, 25%, 0%) */}
          <line
            x1="0"
            y1="24"
            x2="320"
            y2="24"
            stroke="#262C34"
            strokeDasharray="2,3"
            strokeWidth="1"
          />
          <line
            x1="0"
            y1="52"
            x2="320"
            y2="52"
            stroke="#262C34"
            strokeDasharray="2,3"
            strokeWidth="1"
          />
          <line
            x1="0"
            y1="80"
            x2="320"
            y2="80"
            stroke="#262C34"
            strokeDasharray="2,3"
            strokeWidth="1"
          />
          <line
            x1="0"
            y1="98"
            x2="320"
            y2="98"
            stroke="#262C34"
            strokeWidth="1"
          />

          {/* Area Fill */}
          <path d={areaPathStr} fill={`url(#${gradientId})`} />

          {/* Telemetry Line */}
          <polyline
            fill="none"
            stroke={strokeColor}
            strokeWidth="1.75"
            strokeLinecap="round"
            strokeLinejoin="round"
            points={polylineStr}
          />

          {/* Latest Value Pulse Dot */}
          {lastPoint && (
            <circle
              cx={lastPoint.x}
              cy={lastPoint.y}
              r="3"
              fill={strokeColor}
            />
          )}
        </svg>
      </div>

      {/* Footer Axis & Stats */}
      <div className="mt-2 pt-2 border-t border-lunar-border/60 flex items-center justify-between text-[10px] font-mono text-lunar-muted">
        <div className="flex items-center gap-3">
          <span>-30s</span>
          <span>-15s</span>
          <span className="text-lunar-text-sec">Live</span>
        </div>
        <div className="flex items-center gap-3">
          <span>
            AVG{" "}
            <strong className="text-lunar-text-sec font-normal">
              {avgVal.toFixed(0)}%
            </strong>
          </span>
          <span>
            PEAK{" "}
            <strong className="text-lunar-white font-normal">
              {peakVal.toFixed(0)}%
            </strong>
          </span>
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

  return (
    <section className="rounded-lg bg-lunar-surface border border-lunar-border p-5">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2.5">
          <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-text-sec">
            Real-Time System Telemetry Charts
          </h2>
          <span className="w-1.5 h-1.5 rounded-full bg-lunar-healthy animate-pulse" />
        </div>
        <span className="text-[11px] font-mono text-lunar-muted">
          CPU · MEMORY · GPU (30S WINDOW)
        </span>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* CPU Chart */}
        <TelemetryMiniChart
          title="CPU Usage"
          subtitle="Logical Processor Utilization"
          currentPercent={metrics.cpuUsagePercent}
          deltaPercent={metrics.cpuDeltaPercent}
          values={cpuSeries}
          strokeColor="#F5F6F7"
          gradientId="gradCpuChart"
          onClick={onSelectCpu}
        />

        {/* Memory Chart */}
        <TelemetryMiniChart
          title="Memory Usage"
          subtitle={`${formatBytes(metrics.memoryUsedBytes)} / ${formatBytes(
            metrics.memoryTotalBytes
          )}`}
          currentPercent={metrics.memoryUsagePercent}
          deltaPercent={metrics.memoryDeltaPercent}
          values={memSeries}
          strokeColor="#9BAE9F"
          gradientId="gradMemChart"
          onClick={onSelectMemory}
        />

        {/* GPU Chart */}
        <TelemetryMiniChart
          title="GPU Usage"
          subtitle="3D & Compositor Engine Load"
          currentPercent={metrics.gpuUsagePercent ?? 0}
          deltaPercent={metrics.gpuDeltaPercent ?? 0}
          values={gpuSeries}
          strokeColor="#A6A1B8"
          gradientId="gradGpuChart"
          onClick={onSelectGpu}
        />
      </div>
    </section>
  );
};
