import React, { useEffect, useState } from "react";
import { Thermometer } from "lucide-react";
import type { SensorReading, SystemTemperatures } from "../../types/thermal";
import { getSystemTemperatures } from "../../services/tauri";

const POLL_MS = 4000;
const SCALE_MIN_C = 20;
const SCALE_MAX_C = 100;

// Cool to hot. The bar fill reveals this gradient, so the colour at the tip matches the reading.
const TEMP_GRADIENT =
  "linear-gradient(90deg, #3B82F6 0%, #22C55E 32%, #EAB308 58%, #F97316 78%, #EF4444 100%)";

/** Colour of the reading itself, interpolated along the same cool-to-hot scale. */
function tempColor(celsius: number): string {
  const stops: [number, [number, number, number]][] = [
    [30, [59, 130, 246]],
    [50, [34, 197, 94]],
    [65, [234, 179, 8]],
    [80, [249, 115, 22]],
    [95, [239, 68, 68]],
  ];
  if (celsius <= stops[0][0]) return `rgb(${stops[0][1].join(",")})`;
  for (let i = 1; i < stops.length; i++) {
    const [t1, c1] = stops[i];
    const [t0, c0] = stops[i - 1];
    if (celsius <= t1) {
      const f = (celsius - t0) / (t1 - t0);
      const rgb = c0.map((v, k) => Math.round(v + (c1[k] - v) * f));
      return `rgb(${rgb.join(",")})`;
    }
  }
  return `rgb(${stops[stops.length - 1][1].join(",")})`;
}

interface TempRowProps {
  label: string;
  reading: SensorReading | null;
}

const TempRow: React.FC<TempRowProps> = ({ label, reading }) => {
  const celsius = reading?.celsius ?? null;
  const pct =
    celsius === null
      ? 0
      : Math.min(
          100,
          Math.max(3, ((celsius - SCALE_MIN_C) / (SCALE_MAX_C - SCALE_MIN_C)) * 100)
        );

  return (
    <div className="flex items-center gap-3 min-w-0" title={reading?.source}>
      <span className="w-9 shrink-0 text-xs font-semibold text-lunar-text">
        {label}
      </span>

      <div
        className="flex-1 h-2.5 rounded-full bg-lunar-bg border border-lunar-border/80 overflow-hidden"
        role="meter"
        aria-label={`${label} temperature`}
        aria-valuemin={SCALE_MIN_C}
        aria-valuemax={SCALE_MAX_C}
        aria-valuenow={celsius ?? undefined}
      >
        {celsius !== null && (
          <div
            className="h-full rounded-full transition-[width] duration-700 ease-out"
            style={{
              width: `${pct}%`,
              backgroundImage: TEMP_GRADIENT,
              // Keep the gradient anchored to the full track, not squeezed into the fill.
              backgroundSize: `${(100 / pct) * 100}% 100%`,
              backgroundRepeat: "no-repeat",
            }}
          />
        )}
      </div>

      <span
        className="w-14 shrink-0 text-right text-xs font-mono font-semibold"
        style={{ color: celsius === null ? undefined : tempColor(celsius) }}
      >
        {celsius === null ? (
          <span className="text-lunar-muted font-normal">N/A</span>
        ) : (
          `${celsius.toFixed(0)}°C`
        )}
      </span>
    </div>
  );
};

export const TemperaturePanel: React.FC = () => {
  const [temps, setTemps] = useState<SystemTemperatures | null>(null);

  useEffect(() => {
    let alive = true;
    const load = () => {
      getSystemTemperatures()
        .then((t) => {
          if (alive) setTemps(t);
        })
        .catch(() => {
          // Outside Tauri there are no sensors; rows stay on N/A.
        });
    };
    load();
    const timer = window.setInterval(load, POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);

  return (
    <section className="h-full min-h-0 rounded-xl lunar-glass-card p-4 flex flex-col justify-between gap-3">
      <header className="flex items-center gap-2">
        <Thermometer className="w-3.5 h-3.5 text-lunar-text-sec" />
        <h2 className="text-[11px] font-mono uppercase tracking-[0.14em] text-lunar-text-sec">
          Temperature
        </h2>
      </header>

      <div className="flex flex-col justify-around gap-3 flex-1">
        <TempRow label="CPU" reading={temps?.cpu ?? null} />
        <TempRow label="GPU" reading={temps?.gpu ?? null} />
        <TempRow label="RAM" reading={temps?.ram ?? null} />
      </div>

      {temps && (
        <p className="text-[10px] text-lunar-muted truncate">
          N/A means this PC does not expose that sensor. Hover a row for its source.
        </p>
      )}
    </section>
  );
};
