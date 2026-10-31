//! Real hardware temperatures, read without admin rights.
//!
//! * CPU: Windows "Thermal Zone Information" performance counters (ACPI thermal zones, in Kelvin).
//! * GPU: `nvidia-smi` when an NVIDIA driver is installed.
//! * RAM: Windows has no standard memory temperature sensor, so it is reported as unavailable.
//!
//! A sensor that cannot be read is `None`. Nothing here is estimated or hardcoded.

use serde::Serialize;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

#[link(name = "pdh")]
extern "system" {
    fn PdhOpenQueryW(szDataSource: *const u16, dwUserData: usize, phQuery: *mut isize) -> u32;
    fn PdhAddEnglishCounterW(
        hQuery: isize,
        szFullCounterPath: *const u16,
        dwUserData: usize,
        phCounter: *mut isize,
    ) -> u32;
    fn PdhCollectQueryData(hQuery: isize) -> u32;
    fn PdhGetFormattedCounterArrayW(
        hCounter: isize,
        dwFormat: u32,
        lpdwBufferSize: *mut u32,
        lpdwItemCount: *mut u32,
        ItemBuffer: *mut PdhFmtCounterValueItemW,
    ) -> u32;
    fn PdhCloseQuery(hQuery: isize) -> u32;
}

const PDH_FMT_DOUBLE: u32 = 0x0000_0200;
const PDH_MORE_DATA: u32 = 0x8000_07D2;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const KELVIN_OFFSET: f64 = 273.15;

#[repr(C)]
struct PdhFmtCounterValue {
    c_status: u32,
    double_value: f64,
}

#[repr(C)]
struct PdhFmtCounterValueItemW {
    sz_name: *const u16,
    fmt_value: PdhFmtCounterValue,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorReading {
    /// Degrees Celsius, or `None` when this machine does not expose the sensor.
    pub celsius: Option<f32>,
    /// Where the number came from, or why it is missing.
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemTemperatures {
    pub cpu: SensorReading,
    pub gpu: SensorReading,
    pub ram: SensorReading,
}

pub fn read_temperatures() -> SystemTemperatures {
    let cpu = match read_thermal_zones() {
        Some((celsius, zone)) => SensorReading {
            celsius: Some(round1(celsius)),
            source: format!("ACPI thermal zone {}", zone),
        },
        None => SensorReading {
            celsius: None,
            source: "No CPU thermal sensor exposed by Windows on this PC".into(),
        },
    };

    let gpu = match read_nvidia_temperature() {
        Some(celsius) => SensorReading {
            celsius: Some(celsius),
            source: "NVIDIA driver (nvidia-smi)".into(),
        },
        None => SensorReading {
            celsius: None,
            source: "GPU temperature needs an NVIDIA driver (nvidia-smi)".into(),
        },
    };

    let ram = SensorReading {
        celsius: None,
        source: "Windows does not expose a memory temperature sensor".into(),
    };

    SystemTemperatures { cpu, gpu, ram }
}

fn round1(v: f64) -> f32 {
    ((v * 10.0).round() / 10.0) as f32
}

/// Reads a UTF-16 string that PDH placed inside its result buffer.
unsafe fn read_wide(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 && len < 512 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// Hottest plausible thermal zone, preferring a zone whose name mentions the CPU.
fn read_thermal_zones() -> Option<(f64, String)> {
    let mut zones: Vec<(String, f64)> = Vec::new();

    unsafe {
        let mut query: isize = 0;
        if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 || query == 0 {
            return None;
        }

        let path: Vec<u16> = "\\Thermal Zone Information(*)\\Temperature\0"
            .encode_utf16()
            .collect();
        let mut counter: isize = 0;
        if PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) != 0 || counter == 0 {
            PdhCloseQuery(query);
            return None;
        }

        if PdhCollectQueryData(query) == 0 {
            let mut buf_size: u32 = 0;
            let mut item_count: u32 = 0;
            let status = PdhGetFormattedCounterArrayW(
                counter,
                PDH_FMT_DOUBLE,
                &mut buf_size,
                &mut item_count,
                std::ptr::null_mut(),
            );

            if status == PDH_MORE_DATA && buf_size > 0 && item_count > 0 {
                // u64 backing store keeps the buffer 8-byte aligned for the item structs.
                let mut buffer = vec![0u64; (buf_size as usize + 7) / 8];
                let items_ptr = buffer.as_mut_ptr() as *mut PdhFmtCounterValueItemW;
                if PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut buf_size,
                    &mut item_count,
                    items_ptr,
                ) == 0
                {
                    let items = std::slice::from_raw_parts(items_ptr, item_count as usize);
                    for item in items {
                        if item.fmt_value.c_status != 0 {
                            continue;
                        }
                        let celsius = item.fmt_value.double_value - KELVIN_OFFSET;
                        if (1.0..=125.0).contains(&celsius) {
                            zones.push((read_wide(item.sz_name), celsius));
                        }
                    }
                }
            }
        }

        PdhCloseQuery(query);
    }

    if zones.is_empty() {
        return None;
    }

    let cpu_named = zones
        .iter()
        .filter(|(name, _)| name.to_ascii_lowercase().contains("cpu"))
        .max_by(|a, b| a.1.total_cmp(&b.1));
    let best = cpu_named.or_else(|| zones.iter().max_by(|a, b| a.1.total_cmp(&b.1)))?;

    let label = best.0.trim_start_matches('\\').to_string();
    Some((best.1, label))
}

static NVIDIA_SMI_MISSING: AtomicBool = AtomicBool::new(false);

/// Hottest NVIDIA GPU in degrees Celsius, or `None` when no NVIDIA driver is present.
fn read_nvidia_temperature() -> Option<f32> {
    if NVIDIA_SMI_MISSING.load(Ordering::Relaxed) {
        return None;
    }

    let output = match Command::new("nvidia-smi")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "--query-gpu=temperature.gpu",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => {
            // The executable is not installed. Stop trying until the app restarts.
            NVIDIA_SMI_MISSING.store(true, Ordering::Relaxed);
            return None;
        }
    };

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| l.trim().parse::<f32>().ok())
        .filter(|t| (1.0..=130.0).contains(t))
        .max_by(|a, b| a.total_cmp(b))
}
