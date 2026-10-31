// Mirrors src-tauri/src/collector/thermal.rs (serde camelCase).

export interface SensorReading {
  /** Degrees Celsius, or null when this PC does not expose the sensor. */
  celsius: number | null;
  /** Where the number came from, or why it is missing. */
  source: string;
}

export interface SystemTemperatures {
  cpu: SensorReading;
  gpu: SensorReading;
  ram: SensorReading;
}
