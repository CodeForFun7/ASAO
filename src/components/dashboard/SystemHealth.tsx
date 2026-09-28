import React, { useMemo } from "react";
import type {
  SystemMetrics,
  SystemTelemetryPoint,
} from "../../types/process";
import { formatRate } from "../../services/tauri";

interface SystemHealthProps {
  metrics: SystemMetrics;
  history: SystemTelemetryPoint[];
  onSelectCpu: () => void;
  onSelectMemory: () => void;
  onSelectGpu: () => void;
  onSelectNetwork?: () => void;
  onSelectDisk?: () => void;
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

const ActivityColumnChart: React.FC<ActivityColumnChartProps> = ({
  title,
  badgeText,
  currentPercent,
  values,
  strokeColor,
  badgeTextColor = "#0B0D10",
  gradientId,
  peakValue,
  averageValue,
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
    const height = 96;
    const topPad = 24;
    const bottomPad = 8;
    const usableHeight = height - topPad - bottomPad;

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
  }, [values, currentPercent]);

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
      className="group flex flex-col justify-between rounded-lg p-2 -m-2 hover:bg-lunar-elevated/30 transition-colors cursor-pointer"
    >
      {/* Top: Smooth Curve Chart with Floating Usage Callout Badge */}
      <div className="relative h-[96px] w-full mb-2">
        <svg
          viewBox="0 0 240 96"
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

          {/* Highlighted Point Vertical Dashed Line + Usage Callout Badge */}
          {highlightPoint && (
            <g>
              <line
                x1={highlightPoint.x}
                y1={highlightPoint.y + 4}
                x2={highlightPoint.x}
                y2={88}
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
                x={highlightPoint.x - badgeWidth / 2}
                y={Math.max(1, highlightPoint.y - 22)}
                width={badgeWidth}
                height="16"
                rx="8"
                fill={strokeColor}
              />
              <text
                x={highlightPoint.x}
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
      </div>

      {/* Label Centered Below Graph (without duplicate big usage number) */}
      <div className="text-center mb-2">
        <span className="text-xs font-semibold text-lunar-text tracking-wide group-hover:text-lunar-white transition-colors">
          {title}
        </span>
      </div>

      {/* Bottom: Peak & Average Rows */}
      <div className="space-y-1 pt-2 border-t border-lunar-border/50 text-[11px] font-mono">
        <div className="flex items-center justify-between text-lunar-muted">
          <span>Peak</span>
          <span className="text-lunar-text-sec">{peakValue}</span>
        </div>
        <div className="flex items-center justify-between text-lunar-muted">
          <span>Average</span>
          <span className="text-lunar-text-sec">{averageValue}</span>
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
  onSelectNetwork,
  onSelectDisk,
}) => {
  const cpuSeries = useMemo(() => history.map((h) => h.cpuPercent), [history]);
  const memSeries = useMemo(
    () => history.map((h) => h.memoryPercent),
    [history]
  );
  const gpuSeries = useMemo(() => history.map((h) => h.gpuPercent), [history]);
  const netBytesSeries = useMemo(
    () => history.map((h) => h.networkBytesPerSec ?? 0),
    [history]
  );
  const diskBytesSeries = useMemo(
    () => history.map((h) => h.diskBytesPerSec ?? 0),
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

  const gpuStats = useMemo(() => {
    const cur = metrics.gpuUsagePercent ?? 0;
    const arr = gpuSeries.length ? gpuSeries : [cur];
    const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
    const peak = Math.max(...arr, cur);
    return { avg, peak };
  }, [gpuSeries, metrics.gpuUsagePercent]);

  const netStats = useMemo(() => {
    const cur = metrics.networkBytesPerSec ?? 0;
    const arr = netBytesSeries.length ? netBytesSeries : [cur];
    const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
    const peak = Math.max(...arr, cur);
    const scaleMax = Math.max(peak, 256 * 1024); // normalize to 0..100% for curve
    const normalized = arr.map((v) =>
      Math.min(100, Math.max(4, (v / scaleMax) * 85))
    );
    const curPct = Math.min(100, Math.max(4, (cur / scaleMax) * 85));
    return { avg, peak, normalized, curPct, cur };
  }, [netBytesSeries, metrics.networkBytesPerSec]);

  const diskStats = useMemo(() => {
    const cur = metrics.diskBytesPerSec ?? 0;
    const arr = diskBytesSeries.length ? diskBytesSeries : [cur];
    const avg = arr.reduce((a, b) => a + b, 0) / arr.length;
    const peak = Math.max(...arr, cur);
    const scaleMax = Math.max(peak, 1024 * 1024); // normalize to 0..100% for curve
    const normalized = arr.map((v) =>
      Math.min(100, Math.max(4, (v / scaleMax) * 85))
    );
    const curPct = Math.min(100, Math.max(4, (cur / scaleMax) * 85));
    return { avg, peak, normalized, curPct, cur };
  }, [diskBytesSeries, metrics.diskBytesPerSec]);

  return (
    <section className="flex-1 rounded-xl lunar-glass-card p-5 flex flex-col justify-between">
      {/* Top Row Inside Card: CPU & RAM (2 columns as in sketch) */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 md:divide-x md:divide-lunar-border/60">
        <div className="md:pr-3">
          <ActivityColumnChart
            title="CPU"
            badgeText={`${metrics.cpuUsagePercent.toFixed(0)}%`}
            currentPercent={metrics.cpuUsagePercent}
            values={cpuSeries}
            strokeColor="#FFFFFF"
            badgeTextColor="#0E0E0E"
            gradientId="gradCpuActivity"
            peakValue={`${cpuStats.peak.toFixed(0)}%`}
            averageValue={`${cpuStats.avg.toFixed(1)}%`}
            onClick={onSelectCpu}
          />
        </div>

        <div className="md:pl-6">
          <ActivityColumnChart
            title="RAM"
            badgeText={`${metrics.memoryUsagePercent.toFixed(0)}%`}
            currentPercent={metrics.memoryUsagePercent}
            values={memSeries}
            strokeColor="#FFFFFF"
            badgeTextColor="#0E0E0E"
            gradientId="gradRamActivity"
            peakValue={`${memStats.peak.toFixed(0)}%`}
            averageValue={`${memStats.avg.toFixed(1)}%`}
            onClick={onSelectMemory}
          />
        </div>
      </div>

      {/* Bottom Row Inside Card: GPU, Network & Disk (3 columns as in sketch) */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6 pt-5 mt-5 border-t border-lunar-border/60 md:divide-x md:divide-lunar-border/60">
        <div className="md:pr-2">
          <ActivityColumnChart
            title="GPU"
            badgeText={`${(metrics.gpuUsagePercent ?? 0).toFixed(0)}%`}
            currentPercent={metrics.gpuUsagePercent ?? 0}
            values={gpuSeries}
            strokeColor="#FFFFFF"
            badgeTextColor="#0E0E0E"
            gradientId="gradGpuActivity"
            peakValue={`${gpuStats.peak.toFixed(0)}%`}
            averageValue={`${gpuStats.avg.toFixed(1)}%`}
            onClick={onSelectGpu}
          />
        </div>

        <div className="md:px-4">
          <ActivityColumnChart
            title="Network"
            badgeText={formatRate(netStats.cur)}
            currentPercent={netStats.curPct}
            values={netStats.normalized}
            strokeColor="#FFFFFF"
            badgeTextColor="#0E0E0E"
            gradientId="gradNetActivity"
            peakValue={formatRate(netStats.peak)}
            averageValue={formatRate(Math.round(netStats.avg))}
            onClick={onSelectNetwork ?? onSelectCpu}
          />
        </div>

        <div className="md:pl-4">
          <ActivityColumnChart
            title="Disk"
            badgeText={formatRate(diskStats.cur)}
            currentPercent={diskStats.curPct}
            values={diskStats.normalized}
            strokeColor="#FFFFFF"
            badgeTextColor="#0E0E0E"
            gradientId="gradDiskActivity"
            peakValue={formatRate(diskStats.peak)}
            averageValue={formatRate(Math.round(diskStats.avg))}
            onClick={onSelectDisk ?? onSelectCpu}
          />
        </div>
      </div>
    </section>
  );
};


