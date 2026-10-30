// Mirrors src-tauri/src/collector/battery.rs (serde camelCase / kebab-case).

export type BatteryState = "no-battery" | "charging" | "discharging" | "plugged-in" | "unknown";

export interface BatteryStatus {
  hasBattery: boolean;
  state: BatteryState;
  acOnline: boolean | null;
  percent: number | null;
  /** Charge rate (positive) or discharge rate (negative) in watts, from the battery driver. */
  rateWatts: number | null;
  /** Time to empty while discharging. */
  timeRemainingSecs: number | null;
  /** Time to full while charging. */
  timeToFullSecs: number | null;
  remainingMwh: number | null;
  fullChargeMwh: number | null;
  designMwh: number | null;
  /** Full charge capacity as a share of the design capacity. */
  healthPercent: number | null;
  cycleCount: number | null;
  batterySaver: boolean;
}
