import React, { useEffect, useState } from "react";
import { Battery, BatteryCharging } from "lucide-react";
import type { SystemMetrics } from "../../types/process";

interface BatteryManagerLike extends EventTarget {
  charging: boolean;
  level: number;
}

interface NavigatorWithBattery extends Navigator {
  getBattery?: () => Promise<BatteryManagerLike>;
}

interface BatteryStatusCardProps {
  metrics: SystemMetrics;
}

const TOTAL_SEGMENTS = 14;

export const BatteryStatusCard: React.FC<BatteryStatusCardProps> = ({
  metrics,
}) => {
  const [batteryPercent, setBatteryPercent] = useState<number>(80);
  const [isCharging, setIsCharging] = useState<boolean>(true);

  useEffect(() => {
    const nav = navigator as NavigatorWithBattery;
    if (!nav.getBattery) return;

    let batteryRef: BatteryManagerLike | null = null;

    const syncBattery = () => {
      if (!batteryRef) return;
      const pct = Math.round((batteryRef.level ?? 0.8) * 100);
      setBatteryPercent(Math.min(100, Math.max(1, pct)));
      setIsCharging(Boolean(batteryRef.charging));
    };

    void nav
      .getBattery()
      .then((bat) => {
        batteryRef = bat;
        syncBattery();
        bat.addEventListener("levelchange", syncBattery);
        bat.addEventListener("chargingchange", syncBattery);
      })
      .catch(() => {
        // Keep default fallback if battery API is unavailable
      });

    return () => {
      if (batteryRef) {
        batteryRef.removeEventListener("levelchange", syncBattery);
        batteryRef.removeEventListener("chargingchange", syncBattery);
      }
    };
  }, []);

  const filledSegments = Math.max(
    1,
    Math.min(
      TOTAL_SEGMENTS,
      Math.round((batteryPercent / 100) * TOTAL_SEGMENTS)
    )
  );

  // Estimate system power usage in Wh based on CPU, RAM, and GPU load
  const gpuPercent = metrics.gpuUsagePercent ?? 0;
  const estimatedWh = Math.round(
    14 +
      metrics.cpuUsagePercent * 0.32 +
      metrics.memoryUsagePercent * 0.08 +
      gpuPercent * 0.25
  );

  return (
    <div className="rounded-xl lunar-glass-card p-4 flex flex-col justify-between gap-3 shrink-0">
      {/* Top Row: Battery Status (Charging/On Battery) & Percentage */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {isCharging ? (
            <BatteryCharging className="w-4 h-4 text-lunar-white shrink-0" />
          ) : (
            <Battery className="w-4 h-4 text-lunar-white shrink-0" />
          )}
          <span className="text-xs font-medium text-lunar-text">
            Battery Status ({isCharging ? "Charging" : "On Battery"})
          </span>
        </div>
        <span className="text-xs font-semibold font-mono text-lunar-white">
          {batteryPercent}%
        </span>
      </div>

      {/* Bottom Row: Segmented Block Bar + Power Readout */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-1 flex-1">
          {Array.from({ length: TOTAL_SEGMENTS }).map((_, idx) => {
            const isFilled = idx < filledSegments;
            return (
              <div
                key={idx}
                className={`h-5 flex-1 rounded-[3px] transition-colors duration-300 ${
                  isFilled ? "bg-lunar-white" : "bg-[#222222]"
                }`}
              />
            );
          })}
        </div>

        <span className="text-xs font-mono font-medium text-lunar-white shrink-0">
          {estimatedWh} Wh
        </span>
      </div>
    </div>
  );
};
