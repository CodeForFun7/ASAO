import React, { useEffect, useState } from "react";
import { Battery, BatteryCharging, BatteryFull, BatteryLow, Plug } from "lucide-react";
import type { SystemMetrics } from "../../types/process";
import type { BatteryStatus } from "../../types/battery";
import { getBatteryStatus } from "../../services/tauri";

interface BatteryStatusCardProps {
  metrics: SystemMetrics;
}

const TOTAL_SEGMENTS = 14;
const MAX_POWER_W = 80;
const POLL_MS = 5000;

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

function stateLabel(b: BatteryStatus): string {
  switch (b.state) {
    case "charging":
      return "Charging";
    case "discharging":
      return "On battery";
    case "plugged-in":
      return (b.percent ?? 0) >= 99 ? "Fully charged" : "Plugged in, not charging";
    case "no-battery":
      return "No battery";
    default:
      return "Unknown";
  }
}

export const BatteryStatusCard: React.FC<BatteryStatusCardProps> = ({ metrics }) => {
  const [battery, setBattery] = useState<BatteryStatus | null>(null);

  useEffect(() => {
    let alive = true;
    const load = () => {
      getBatteryStatus()
        .then((b) => {
          if (alive) setBattery(b);
        })
        .catch(() => {
          // Outside Tauri: the card shows "Reading battery"
        });
    };
    load();
    const timer = window.setInterval(load, POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);

  // Estimated system power draw from CPU, GPU, RAM and I/O load. Used when the battery
  // driver reports no rate (plugged in, or no battery).
  const gpuPercent = metrics.gpuUsagePercent ?? 0;
  const diskMb = (metrics.diskBytesPerSec ?? 0) / (1024 * 1024);
  const netMb = (metrics.networkBytesPerSec ?? 0) / (1024 * 1024);
  const estimatedWatts = Math.min(
    MAX_POWER_W,
    Math.max(
      8,
      Math.round(
        10 +
          metrics.cpuUsagePercent * 0.42 +
          gpuPercent * 0.28 +
          metrics.memoryUsagePercent * 0.12 +
          Math.min(8, (diskMb + netMb) * 0.5)
      )
    )
  );

  // Measured rate: while discharging it is the real system draw; while charging, the charge rate.
  const rate = battery?.rateWatts ?? null;
  const measured = rate !== null;
  const shownWatts = measured ? Math.abs(rate) : estimatedWatts;
  const powerLabel = measured
    ? rate > 0
      ? `+${Math.abs(rate).toFixed(1)} W`
      : `${Math.abs(rate).toFixed(1)} W`
    : `~${estimatedWatts} W`;
  const powerTitle = measured
    ? rate > 0
      ? "Charging rate reported by the battery"
      : "Power drawn from the battery right now"
    : "Estimated from current CPU, GPU, memory and disk load";

  // Segmented bar depicts how much power is being consumed (not battery level %)
  const powerConsumptionRatio = Math.min(1, Math.max(0.08, shownWatts / MAX_POWER_W));
  const filledSegments = Math.max(1, Math.min(TOTAL_SEGMENTS, Math.round(powerConsumptionRatio * TOTAL_SEGMENTS)));

  const hasBattery = battery?.hasBattery ?? false;
  const percent = battery?.percent ?? null;
  const Icon = !battery || !hasBattery
    ? Plug
    : battery.state === "charging"
      ? BatteryCharging
      : battery.state === "plugged-in" && (percent ?? 0) >= 99
        ? BatteryFull
        : percent !== null && percent <= 20 && battery.state === "discharging"
          ? BatteryLow
          : Battery;
  const low = hasBattery && battery?.state === "discharging" && percent !== null && percent <= 20;

  const details: string[] = [];
  if (battery && hasBattery) {
    if (battery.timeRemainingSecs) details.push(`${formatDuration(battery.timeRemainingSecs)} left`);
    if (battery.timeToFullSecs) details.push(`Full in ${formatDuration(battery.timeToFullSecs)}`);
    if (battery.healthPercent !== null) details.push(`Health ${battery.healthPercent}%`);
    if (battery.cycleCount) details.push(`${battery.cycleCount} cycles`);
    if (battery.batterySaver) details.push("Battery saver on");
  } else if (battery) {
    details.push("Running on AC power");
  }

  return (
    <div className="rounded-xl lunar-glass-card p-4 flex flex-col justify-between gap-3 shrink-0">
      {/* Top Row: battery state & percentage */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <Icon className={`w-4 h-4 shrink-0 ${low ? "text-lunar-critical" : "text-lunar-white"}`} />
          <span className="text-xs font-medium text-lunar-text truncate">
            {battery ? `Battery Status (${stateLabel(battery)})` : "Reading battery"}
          </span>
        </div>
        {hasBattery && percent !== null && (
          <span className={`text-xs font-semibold font-mono ${low ? "text-lunar-critical" : "text-lunar-white"}`}>
            {percent}%
          </span>
        )}
      </div>

      {/* Bottom Row: Segmented Block Bar + Power Readout */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-1 flex-1">
          {Array.from({ length: TOTAL_SEGMENTS }).map((_, idx) => (
            <div
              key={idx}
              className={`h-5 flex-1 rounded-[3px] transition-colors duration-300 ${
                idx < filledSegments ? "bg-lunar-white" : "bg-lunar-elevated"
              }`}
            />
          ))}
        </div>

        <span className="text-xs font-mono font-medium text-lunar-white shrink-0" title={powerTitle}>
          {powerLabel}
        </span>
      </div>

      {details.length > 0 && (
        <div className="text-[10px] text-lunar-text-sec truncate">{details.join(" · ")}</div>
      )}
    </div>
  );
};
