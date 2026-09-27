import React, { useMemo } from "react";
import { ArrowUpRight } from "lucide-react";
import type {
  ProcessInfo,
  SystemMetrics,
  SystemTelemetryPoint,
} from "../../types/process";
import { formatBytes, formatRate } from "../../services/tauri";

interface SystemHealthProps {
  metrics: SystemMetrics;
  history: SystemTelemetryPoint[];
  processes: ProcessInfo[];
  onSelectCpu: () => void;
  onSelectMemory: () => void;
  onSelectGpu: () => void;
  onSelectProcesses: () => void;
}

interface MetricAndChartCardProps {
  title: string;
  primaryValue: string;
  secondaryValue: string;
  values: number[]; // normalized 0..100 for chart curve
  badgeText: string;
  strokeColor: string;
  gradientId: string;
  statLeftLabel: string;
  statLeftValue: string;
  statRightLabel: string;
  statRightValue: string;
  onClick: () => void;
}

function buildSmoothCurvePath(coords: { x: number; y: number }[]): string {
  if (coords.length === 0) return "";
  if (coords.length === 1) {
    return `M ${coords[0].x.toFixed(1)},${coords[0].y.toFixed(1)}`;
  }

  let d = `M ${coords[0].x.toFixed(1)},${coords[0].y.toFixed(1)}`;
  for (let i = 0; i < coords.length - 1; i++) {
    const p0 = coords[i];
    const p1 = coords[i + 1];
    const midX = (p0.x + p1.x) / 2;
    d += ` C ${midX.toFixed(1)},${p0.y.toFixed(1)} ${midX.toFixed(1)},${p1.y.toFixed(1)} ${p1.x.toFixed(1)},${p1.y.toFixed(1)}`;
  }
  return d;
}

const MetricAndChartCard: React.FC<MetricAndChartCardProps> = ({
  title,
  primaryValue,
  secondaryValue,
  values,
  badgeText,
  strokeColor,
  gradientId,
  statLeftLabel,
  statLeftValue,
  statRightLabel,
  statRightValue,
  onClick,
}) => {
  const { curvePath, areaPath, highlightPoint } = useMemo(() => {
    const targetCount = 18;
    const fillVal = values[values.length - 1] ?? 10;
    const sampled =
      values.length < targetCount
        ? [...Array(targetCount - values.length).fill(fillVal), ...values]
        : values.slice(values.length - targetCount);

    const width = 240;
    const height = 84;
    const topPad = 22;
    const bottomPad = 6;
    const usableHeight = height - topPad - bottomPad;

    const coords = sampled.map((v, i) => {
      const x = (i / (targetCount - 1)) * width;
      const clamped = Math.min(100, Math.max(2, v));
      const y = topPad + usableHeight - (clamped / 100) * usableHeight;
      return { x, y, val: clamped };
    });

    let peakIdx = Math.floor(targetCount * 0.65);
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
  }, [values]);

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
      className="group rounded-xl lunar-glass-sub p-4 flex flex-col justify-between hover:border-lunar-white/20 transition-all cursor-pointer"
    >
      {/* Top: Metric Header & Primary Value */}
      <div className="flex items-start justify-between gap-2 mb-2">
        <div>
          <div className="flex items-center gap-1.5">
            <span
              className="w-2 h-2 rounded-full shrink-0"
              style={{ backgroundColor: strokeColor }}
            />
            <span className="text-xs font-medium text-lunar-text-sec group-hover:text-lunar-white transition-colors">
              {title}
            </span>
          </div>
          <div className="text-2xl font-semibold text-lunar-white font-mono tracking-tight mt-1">
            {primaryValue}
          </div>
        </div>

        <div className="text-right flex flex-col items-end">
          <ArrowUpRight className="w-3.5 h-3.5 text-lunar-muted group-hover:text-lunar-white transition-colors" />
          <span className="text-[11px] font-mono text-lunar-muted mt-1">
            {secondaryValue}
          </span>
        </div>
      </div>

      {/* Middle: Smooth Telemetry Chart */}
      <div className="relative h-[84px] w-full my-1">
        <svg
          viewBox="0 0 240 84"
          preserveAspectRatio="none"
          className="w-full h-full overflow-visible"
        >
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={strokeColor} stopOpacity="0.25" />
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
                y2={78}
                stroke={strokeColor}
                strokeWidth="1.2"
                strokeDasharray="2,4"
                strokeOpacity="0.7"
              />
              <circle
                cx={highlightPoint.x}
                cy={highlightPoint.y}
                r="3.5"
                fill={strokeColor}
                stroke="#0B0D10"
                strokeWidth="1.5"
              />
              <rect
                x={highlightPoint.x - 21}
                y={Math.max(0, highlightPoint.y - 20)}
                width="42"
                height="15"
                rx="7.5"
                fill={strokeColor}
              />
              <text
                x={highlightPoint.x}
                y={Math.max(0, highlightPoint.y - 20) + 10.5}
                textAnchor="middle"
                fill="#0B0D10"
                fontSize="9"
                fontWeight="700"
                fontFamily="Plus Jakarta Sans, sans-serif"
              >
                {badgeText}
              </text>
            </g>
          )}
        </svg>
      </div>

      {/* Bottom: Summary Stats */}
      <div className="pt-2 border-t border-lunar-border/50 flex items-center justify-between text-[11px] font-mono text-lunar-muted">
        <span>
          {statLeftLabel}:{" "}
          <strong className="text-lunar-text-sec font-normal">
            {statLeftValue}
          </strong>
        </span>
        <span>
          {statRightLabel}:{" "}
          <strong className="text-lunar-text-sec font-normal">
            {statRightValue}
          </strong>
        </span>
      </div>
    </div>
  );
};

export const SystemHealth: React.FC<SystemHealthProps> = ({
  metrics,
  history,
  processes,
  onSelectCpu,
  onSelectMemory,
  onSelectGpu,
  onSelectProcesses,
}) => {
  const cpuSeries = useMemo(() => history.map((h) => h.cpuPercent), [history]);
  const memSeries = useMemo(
    () => history.map((h) => h.memoryPercent),
    [history]
  );
  const gpuSeries = useMemo(() => history.map((h) => h.gpuPercent), [history]);

  // Compute live & historical Disk and Network rates
  const {
    currentDiskBps,
    peakDiskBps,
    diskNormalizedSeries,
    currentNetBps,
    peakNetBps,
    netNormalizedSeries,
  } = useMemo(() => {
    let liveDisk = 0;
    let liveNet = 0;
    for (const p of processes) {
      liveDisk += p.diskBytesPerSec || 0;
      liveNet += p.networkBytesPerSec || 0;
    }

    const rawDiskHistory = history.map((h) => h.diskBytesPerSec ?? liveDisk);
    const rawNetHistory = history.map((h) => h.networkBytesPerSec ?? liveNet);

    const diskArr = rawDiskHistory.length ? rawDiskHistory : [liveDisk];
    const netArr = rawNetHistory.length ? rawNetHistory : [liveNet];

    const maxDisk = Math.max(...diskArr, liveDisk, 1024 * 1024); // min 1 MB/s ceiling for normalization
    const maxNet = Math.max(...netArr, liveNet, 256 * 1024); // min 256 KB/s ceiling for normalization

    const diskNorm = diskArr.map((v) =>
      Math.min(96, Math.max(6, (v / maxDisk) * 85))
    );
    const netNorm = netArr.map((v) =>
      Math.min(96, Math.max(6, (v / maxNet) * 85))
    );

    return {
      currentDiskBps: liveDisk,
      peakDiskBps: Math.max(...diskArr, liveDisk),
      diskNormalizedSeries: diskNorm,
      currentNetBps: liveNet,
      peakNetBps: Math.max(...netArr, liveNet),
      netNormalizedSeries: netNorm,
    };
  }, [history, processes]);

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
    <section className="h-full rounded-2xl lunar-glass-card p-4 flex flex-col gap-4 justify-between">
      {/* Row 1: CPU Metric & Chart (Left) | RAM Metric & Chart (Right) */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 flex-1">
        <MetricAndChartCard
          title="CPU Metric & Chart"
          primaryValue={`${metrics.cpuUsagePercent.toFixed(0)}%`}
          secondaryValue={`${
            metrics.cpuDeltaPercent >= 0 ? "+" : ""
          }${metrics.cpuDeltaPercent.toFixed(1)}% vs avg`}
          values={cpuSeries}
          badgeText={`${metrics.cpuUsagePercent.toFixed(0)}%`}
          strokeColor="#9BAE9F"
          gradientId="gradCpuBox"
          statLeftLabel="Peak"
          statLeftValue={`${cpuStats.peak.toFixed(0)}%`}
          statRightLabel="Avg"
          statRightValue={`${cpuStats.avg.toFixed(1)}%`}
          onClick={onSelectCpu}
        />

        <MetricAndChartCard
          title="RAM Metric & Chart"
          primaryValue={`${metrics.memoryUsagePercent.toFixed(0)}%`}
          secondaryValue={`${formatBytes(
            metrics.memoryUsedBytes
          )} / ${formatBytes(metrics.memoryTotalBytes)}`}
          values={memSeries}
          badgeText={`${metrics.memoryUsagePercent.toFixed(0)}%`}
          strokeColor="#A6A1B8"
          gradientId="gradRamBox"
          statLeftLabel="Used"
          statLeftValue={formatBytes(metrics.memoryUsedBytes)}
          statRightLabel="Avg"
          statRightValue={`${memStats.avg.toFixed(1)}%`}
          onClick={onSelectMemory}
        />
      </div>

      {/* Row 2: GPU Metric & Chart | Network Metric & Chart | Disk Metric & Chart */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 flex-1">
        <MetricAndChartCard
          title="GPU Metric & Chart"
          primaryValue={`${(metrics.gpuUsagePercent ?? 0).toFixed(0)}%`}
          secondaryValue="3D & Compositor"
          values={gpuSeries}
          badgeText={`${(metrics.gpuUsagePercent ?? 0).toFixed(0)}%`}
          strokeColor="#C9A66B"
          gradientId="gradGpuBox"
          statLeftLabel="Peak"
          statLeftValue={`${gpuStats.peak.toFixed(0)}%`}
          statRightLabel="Avg"
          statRightValue={`${gpuStats.avg.toFixed(1)}%`}
          onClick={onSelectGpu}
        />

        <MetricAndChartCard
          title="Network Metric & Chart"
          primaryValue={formatRate(currentNetBps)}
          secondaryValue="Active Socket I/O"
          values={netNormalizedSeries}
          badgeText={formatRate(currentNetBps)}
          strokeColor="#7DAEA3"
          gradientId="gradNetBox"
          statLeftLabel="Peak"
          statLeftValue={formatRate(peakNetBps)}
          statRightLabel="State"
          statRightValue={currentNetBps > 32 * 1024 ? "Active" : "Idle"}
          onClick={onSelectProcesses}
        />

        <MetricAndChartCard
          title="Disk Metric & Chart"
          primaryValue={formatRate(currentDiskBps)}
          secondaryValue="Read / Write Rate"
          values={diskNormalizedSeries}
          badgeText={formatRate(currentDiskBps)}
          strokeColor="#D49A89"
          gradientId="gradDiskBox"
          statLeftLabel="Peak"
          statLeftValue={formatRate(peakDiskBps)}
          statRightLabel="State"
          statRightValue={currentDiskBps > 64 * 1024 ? "Active" : "Idle"}
          onClick={onSelectProcesses}
        />
      </div>
    </section>
  );
};


