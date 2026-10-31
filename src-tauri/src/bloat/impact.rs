//! `ImpactScorer`: 0-100 points from startup, background CPU, memory, auto-start services and
//! tasks, and disk footprint. `ResourceSampler` averages CPU over a window from ASAO's process
//! monitor. `StartupImpactReader` reads Windows' own startup measurements when it can.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::thresholds::*;
use super::{Impact, ImpactBand, ImpactPart};
use crate::analyzer::ProcessInfo;
use crate::residual::normalize::path_key;
use crate::residual::sizer::FolderSize;

/// Inputs for one item, already measured.
#[derive(Debug, Clone, Default)]
pub struct ImpactInput {
    /// Enabled startup entries: Windows' measurement when known, else `None` per entry.
    pub startup: Vec<Option<(u64, u64)>>,
    /// Scheduled tasks that run at logon or boot (count as "runs at startup").
    pub logon_tasks: u32,
    /// Average CPU % of the whole machine over the sampling window, if sampled.
    pub cpu_percent: Option<f64>,
    pub sample_seconds: u32,
    pub memory_bytes: u64,
    pub auto_services: u32,
    pub frequent_tasks: u32,
    pub disk_bytes: Option<u64>,
}

pub fn startup_bucket(cpu_ms: u64, disk_bytes: u64) -> (u32, &'static str) {
    if cpu_ms > STARTUP_HIGH_CPU_MS || disk_bytes > STARTUP_HIGH_DISK_BYTES {
        (STARTUP_HIGH_POINTS, "High")
    } else if cpu_ms >= STARTUP_MEDIUM_CPU_MS || disk_bytes >= STARTUP_MEDIUM_DISK_BYTES {
        (STARTUP_MEDIUM_POINTS, "Medium")
    } else {
        (STARTUP_LOW_POINTS, "Low")
    }
}

fn step<T: PartialOrd + Copy>(value: T, steps: &[(T, u32)]) -> u32 {
    steps.iter().find(|(min, _)| value >= *min).map(|(_, p)| *p).unwrap_or(0)
}

pub fn format_bytes(b: u64) -> String {
    let (v, unit) = if b >= 1 << 30 {
        (b as f64 / (1u64 << 30) as f64, "GB")
    } else if b >= 1 << 20 {
        (b as f64 / (1u64 << 20) as f64, "MB")
    } else if b >= 1 << 10 {
        (b as f64 / 1024.0, "KB")
    } else {
        return format!("{} B", b);
    };
    if v >= 100.0 {
        format!("{:.0} {}", v, unit)
    } else {
        format!("{:.1} {}", v, unit)
    }
}

pub fn band_for(total: u32) -> ImpactBand {
    if total >= BAND_HIGH {
        ImpactBand::High
    } else if total >= BAND_MEDIUM {
        ImpactBand::Medium
    } else {
        ImpactBand::Low
    }
}

pub fn score(input: &ImpactInput) -> Impact {
    // Startup: the heaviest enabled entry decides; logon tasks count as "runs at startup".
    let mut startup = ImpactPart { max: STARTUP_MAX, ..Default::default() };
    let mut best: Option<(u32, String)> = None;
    for e in &input.startup {
        let (pts, text) = match e {
            Some((cpu, disk)) => {
                let (p, label) = startup_bucket(*cpu, *disk);
                (p, format!("{} startup impact ({} ms CPU, {} disk)", label, cpu, format_bytes(*disk)))
            }
            None => (STARTUP_MEDIUM_POINTS, "Runs at startup".to_string()),
        };
        if best.as_ref().map(|b| pts > b.0).unwrap_or(true) {
            best = Some((pts, text));
        }
    }
    if best.is_none() && input.logon_tasks > 0 {
        best = Some((STARTUP_MEDIUM_POINTS, "A scheduled task runs at sign-in".into()));
    }
    match best {
        Some((p, t)) => {
            startup.points = p.min(STARTUP_MAX);
            startup.value = t;
        }
        None => startup.value = "Doesn't run at startup".into(),
    }

    let cpu = match input.cpu_percent {
        Some(c) => ImpactPart {
            points: step(c, CPU_STEPS).min(CPU_MAX),
            max: CPU_MAX,
            value: format!("{:.1}% average CPU over {} s", c, input.sample_seconds),
        },
        None => ImpactPart { points: 0, max: CPU_MAX, value: "Not running".into() },
    };

    let memory = ImpactPart {
        points: step(input.memory_bytes, MEMORY_STEPS).min(MEMORY_MAX),
        max: MEMORY_MAX,
        value: if input.memory_bytes == 0 {
            "Not running".into()
        } else {
            format!("{} in use", format_bytes(input.memory_bytes))
        },
    };

    let count = input.auto_services + input.frequent_tasks;
    let autostart = ImpactPart {
        points: step(count, AUTOSTART_STEPS).min(AUTOSTART_MAX),
        max: AUTOSTART_MAX,
        value: match (input.auto_services, input.frequent_tasks) {
            (0, 0) => "No automatic services or frequent tasks".into(),
            (s, t) => format!(
                "{} automatic {}, {} frequent scheduled {}",
                s,
                if s == 1 { "service" } else { "services" },
                t,
                if t == 1 { "task" } else { "tasks" }
            ),
        },
    };

    let disk = match input.disk_bytes {
        Some(b) => ImpactPart {
            points: step(b, DISK_STEPS).min(DISK_MAX),
            max: DISK_MAX,
            value: format!("{} on disk", format_bytes(b)),
        },
        None => ImpactPart { points: 0, max: DISK_MAX, value: "Size unknown".into() },
    };

    let total = (startup.points + cpu.points + memory.points + autostart.points + disk.points).min(100);
    Impact {
        total,
        band: band_for(total),
        startup,
        cpu,
        memory,
        autostart,
        disk,
    }
}

/// Per-process averages over the sampling window, keyed by process id.
#[derive(Debug, Clone, Default)]
pub struct ProcessSample {
    pub exe_key: Option<String>,
    pub cpu_sum: f64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct Samples {
    pub count: u32,
    pub seconds: u32,
    pub by_pid: HashMap<u32, ProcessSample>,
}

impl Samples {
    /// Average CPU % and current memory of the processes that pass `pick`.
    pub fn totals(&self, pick: impl Fn(u32, Option<&str>) -> bool) -> (Option<f64>, u64) {
        let mut cpu = 0.0;
        let mut mem = 0u64;
        let mut any = false;
        for (pid, s) in &self.by_pid {
            if pick(*pid, s.exe_key.as_deref()) {
                any = true;
                cpu += s.cpu_sum;
                mem = mem.saturating_add(s.memory_bytes);
            }
        }
        if !any {
            return (None, 0);
        }
        (Some(cpu / self.count.max(1) as f64), mem)
    }
}

/// `ResourceSampler`: takes a process snapshot every couple of seconds for `seconds`.
/// `progress` gets the percentage done. Stops early when `cancel` is set.
pub fn sample(
    seconds: u32,
    snapshot: &dyn Fn() -> Vec<ProcessInfo>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u8),
) -> Samples {
    let mut out = Samples { seconds, ..Default::default() };
    let rounds = ((seconds as u64 * 1000) / SAMPLE_INTERVAL_MS).max(1) as u32;
    for i in 0..rounds {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        for p in snapshot() {
            let e = out.by_pid.entry(p.pid).or_insert_with(|| ProcessSample {
                exe_key: p.executable_path.as_deref().map(path_key),
                ..Default::default()
            });
            e.cpu_sum += p.cpu_percent as f64;
            e.memory_bytes = p.memory_bytes;
        }
        out.count += 1;
        progress(((i + 1) * 100 / rounds) as u8);
        if i + 1 < rounds {
            std::thread::sleep(Duration::from_millis(SAMPLE_INTERVAL_MS));
        }
    }
    out
}

/// `StartupImpactReader`: Windows' startup measurements from
/// `%WINDIR%\System32\WDI\LogFiles\StartupInfo\*.xml` (readable as administrator).
/// Returns CPU ms and disk bytes per program path key, the largest seen per program.
pub fn read_startup_info(dir: &Path) -> Option<HashMap<String, (u64, u64)>> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut out: HashMap<String, (u64, u64)> = HashMap::new();
    let mut any = false;
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().map(|e| !e.eq_ignore_ascii_case("xml")).unwrap_or(true) {
            continue;
        }
        let Ok(bytes) = std::fs::read(&p) else { continue };
        any = true;
        let text = decode_xml(&bytes);
        for (path, cpu, disk) in parse_startup_xml(&text) {
            let e = out.entry(path_key(&path)).or_insert((0, 0));
            e.0 = e.0.max(cpu);
            e.1 = e.1.max(disk);
        }
    }
    any.then_some(out)
}

fn decode_xml(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).to_string()
    }
}

fn tag_value(block: &str, tag: &str) -> Option<u64> {
    let open = format!("<{}>", tag);
    let start = block.find(&open)? + open.len();
    let end = block[start..].find('<')? + start;
    block[start..end].trim().parse().ok()
}

/// `<Process Name="C:\...\foo.exe" ...> ... <CpuUsage>123</CpuUsage> <DiskUsage>4567</DiskUsage>`
pub fn parse_startup_xml(text: &str) -> Vec<(String, u64, u64)> {
    let mut out = Vec::new();
    for block in text.split("<Process ").skip(1) {
        let Some(name_start) = block.find("Name=\"") else { continue };
        let rest = &block[name_start + 6..];
        let Some(name_end) = rest.find('"') else { continue };
        let name = &rest[..name_end];
        let body = block.split("</Process>").next().unwrap_or(block);
        let cpu = tag_value(body, "CpuUsage").unwrap_or(0);
        let disk = tag_value(body, "DiskUsage").unwrap_or(0);
        out.push((name.to_string(), cpu, disk));
    }
    out
}

/// Disk footprint: the registry estimate when there is one, otherwise the measured folder.
pub fn footprint(estimated: Option<u64>, measured: Option<FolderSize>) -> Option<u64> {
    estimated
        .filter(|b| *b > 0)
        .or_else(|| measured.map(|m| m.size_on_disk).filter(|b| *b > 0))
}
