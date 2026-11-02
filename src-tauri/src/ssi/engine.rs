//! The System Strain Index engine.
//!
//! SSI = 0.15·CPU + 0.15·Memory + 0.08·GPU + 0.08·Disk + 0.15·Background + 0.15·Thermal
//!     + 0.10·Startup + 0.08·Storage + 0.06·Boot
//!
//! Each factor is first normalised to 0..100 (see `pressure`) and smoothed with a rolling
//! average. A factor that cannot be measured on this PC is dropped and its weight is shared
//! proportionally by the factors that can, so the weights used always add up to 1.0.

use std::collections::{HashMap, VecDeque};

use crate::analyzer::{ProcessActivityState, ProcessCategory, ProcessInfo, ProcessStatus, SystemMetrics};
use crate::collector::extras::{ExtraTelemetry, SensorState};

use super::background::{self, display_name, BackgroundAnalysis};
use super::context::{BootState, StartupSummary};
use super::events;
use super::model::*;
use super::pressure::{self, mean, percentile90, CPU_MARGINS, GPU_MARGINS};
use super::recommend::{self, RecommendationInputs};

const TELEMETRY_CAPACITY: usize = 3600;
const CPU_WINDOW_SECS: usize = 30;
const TREND_WINDOW_SECS: u64 = 60;
const TREND_THRESHOLD: f32 = 3.0;
const LEVEL_CONFIRM_SECS: u32 = 10;
const EVENT_COOLDOWN_MS: u64 = 10 * 60 * 1000;

pub struct SsiInputs<'a> {
    pub now_ms: u64,
    pub metrics: &'a SystemMetrics,
    pub processes: &'a [ProcessInfo],
    pub extras: &'a ExtraTelemetry,
    pub startup: Option<&'a StartupSummary>,
    pub drives: &'a [DriveInfo],
    pub composition: Option<&'a StorageComposition>,
    pub boot: &'a BootState,
    pub device_name: &'a str,
    pub uptime_seconds: u64,
}

/// One factor before smoothing and weighting.
struct RawFactor {
    score: Option<f32>,
    summary: String,
    measurements: Vec<Measurement>,
    unavailable_reason: Option<String>,
}

impl RawFactor {
    fn measured(score: f32, summary: String, measurements: Vec<Measurement>) -> Self {
        Self { score: Some(score), summary, measurements, unavailable_reason: None }
    }

    fn unavailable(reason: &str, measurements: Vec<Measurement>) -> Self {
        Self {
            score: None,
            summary: reason.to_string(),
            measurements,
            unavailable_reason: Some(reason.to_string()),
        }
    }
}

#[derive(Default)]
struct LevelTracker {
    stable: Option<SsiLevel>,
    candidate: Option<(SsiLevel, u32)>,
}

impl LevelTracker {
    /// Reports a change only after the new level has held for several seconds.
    fn observe(&mut self, level: SsiLevel) -> Option<(SsiLevel, SsiLevel)> {
        let Some(stable) = self.stable else {
            self.stable = Some(level);
            return None;
        };
        if stable == level {
            self.candidate = None;
            return None;
        }
        let count = match self.candidate {
            Some((c, n)) if c == level => n + 1,
            _ => 1,
        };
        if count >= LEVEL_CONFIRM_SECS {
            self.stable = Some(level);
            self.candidate = None;
            return Some((stable, level));
        }
        self.candidate = Some((level, count));
        None
    }
}

pub struct SsiEngine {
    factor_windows: HashMap<FactorId, VecDeque<f32>>,
    cpu_window: VecDeque<f32>,
    score_history: VecDeque<(u64, f32)>,
    telemetry: VecDeque<TelemetryPoint>,
    level: LevelTracker,
    cooldowns: HashMap<String, u64>,
    was_throttling: bool,
    announced_start: bool,
    latest: Option<DashboardSnapshot>,
}

impl Default for SsiEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SsiEngine {
    pub fn new() -> Self {
        Self {
            factor_windows: HashMap::new(),
            cpu_window: VecDeque::with_capacity(CPU_WINDOW_SECS + 1),
            score_history: VecDeque::with_capacity(160),
            telemetry: VecDeque::with_capacity(TELEMETRY_CAPACITY),
            level: LevelTracker::default(),
            cooldowns: HashMap::new(),
            was_throttling: false,
            announced_start: false,
            latest: None,
        }
    }

    pub fn latest(&self) -> Option<DashboardSnapshot> {
        self.latest.clone()
    }

    /// Recorded CPU, memory, GPU and SSI values, oldest first.
    pub fn history(&self) -> Vec<TelemetryPoint> {
        self.telemetry.iter().copied().collect()
    }

    pub fn update(&mut self, input: &SsiInputs) -> DashboardSnapshot {
        let m = input.metrics;
        let bg = background::analyze(input.processes);

        // Split each shared resource into essential load and avoidable background load, so
        // that nothing is counted by two factors.
        let cpu_total = m.cpu_usage_percent;
        let bg_cpu = bg.avoidable_cpu_percent.min(cpu_total * 0.9);
        let essential_cpu = (cpu_total - bg_cpu).max(0.0);

        let total_ram = m.memory_total_bytes.max(1);
        let ram_used_pct = m.memory_usage_percent;
        let bg_ram_pct = (bg.avoidable_memory_bytes as f32 / total_ram as f32 * 100.0).min(ram_used_pct * 0.9);
        let essential_ram_pct = (ram_used_pct - bg_ram_pct).max(0.0);

        self.cpu_window.push_back(essential_cpu);
        while self.cpu_window.len() > CPU_WINDOW_SECS {
            self.cpu_window.pop_front();
        }

        let (thermal_raw, thermal_status) = thermal_factor(input.extras);

        let raw: Vec<(FactorId, RawFactor)> = vec![
            (FactorId::Cpu, self.cpu_factor(cpu_total, bg_cpu)),
            (FactorId::Memory, memory_factor(m, input.extras, essential_ram_pct, bg_ram_pct)),
            (FactorId::Gpu, gpu_factor(m, input.extras)),
            (FactorId::DiskIo, disk_factor(input.extras, bg.avoidable_disk_share)),
            (FactorId::Background, background_factor(&bg, bg_cpu, bg_ram_pct, total_ram)),
            (FactorId::Thermal, thermal_raw),
            (FactorId::Startup, startup_factor(input.startup)),
            (FactorId::Storage, storage_factor(input.drives)),
            (FactorId::Boot, boot_factor(input.boot)),
        ];

        let mut factors = self.weigh(raw);
        let ssi_float: f32 = factors.iter().map(|f| f.contribution).sum();
        let score = ssi_float.round().clamp(0.0, 100.0) as u8;
        let level = SsiLevel::from_score(score);
        let available_weight: f32 = factors.iter().filter(|f| f.score.is_some()).map(|f| f.weight).sum();

        let trend = self.trend(input.now_ms, ssi_float);
        let summary = ssi_summary(level, &factors);

        let top = top_processes(input.processes, input.extras, total_ram);
        let primary_drive = input.drives.iter().find(|d| d.is_system).or_else(|| input.drives.first());

        let recommendations = recommend::build(&RecommendationInputs {
            factors: &factors,
            background: &bg,
            startup: input.startup,
            primary_drive,
            thermal: &thermal_status,
            top: &top,
            total_memory_bytes: total_ram,
        });

        let performance = performance_impact(&factors, &bg, input.startup, ssi_float, bg_cpu, bg_ram_pct, total_ram);

        self.record_events(input.now_ms, level, score, &factors, &top, &thermal_status);

        let point = TelemetryPoint {
            timestamp_ms: input.now_ms,
            cpu: round1(cpu_total),
            memory: round1(ram_used_pct),
            gpu: m.gpu_usage_percent.map(round1),
            ssi: round1(ssi_float),
        };
        self.telemetry.push_back(point);
        while self.telemetry.len() > TELEMETRY_CAPACITY {
            self.telemetry.pop_front();
        }

        // Keep the factors in the fixed order of the SSI definition.
        factors.sort_by_key(|f| FactorId::ALL.iter().position(|id| *id == f.id).unwrap_or(usize::MAX));

        let snapshot = DashboardSnapshot {
            timestamp_ms: input.now_ms,
            ssi: SsiResult {
                score,
                level,
                label: level.label().to_string(),
                summary,
                trend,
                factors,
                available_weight,
                timestamp_ms: input.now_ms,
            },
            performance,
            top_processes: top,
            storage: StorageOverview {
                primary: primary_drive.cloned(),
                drives: input.drives.to_vec(),
                composition: input.composition.cloned(),
            },
            thermal: thermal_status,
            recommendations,
            events: events::recent(10),
            point,
            device: DeviceInfo {
                name: input.device_name.to_string(),
                process_count: m.total_processes,
                uptime_seconds: input.uptime_seconds,
            },
        };
        self.latest = Some(snapshot.clone());
        snapshot
    }

    // -- factors -----------------------------------------------------------------------

    fn cpu_factor(&self, cpu_total: f32, bg_cpu: f32) -> RawFactor {
        let samples: Vec<f32> = self.cpu_window.iter().copied().collect();
        let sustained = mean(samples.iter().copied()).unwrap_or(0.0);
        let p90 = percentile90(&samples).unwrap_or(sustained);
        let score = pressure::cpu_pressure(sustained, p90);
        let summary = if score < 20.0 {
            format!("The CPU has plenty of headroom, averaging {:.0}% over the last {} s.", sustained, samples.len())
        } else if score < 55.0 {
            format!("Averaging {:.0}% over {} s, with busy moments near {:.0}%.", sustained, samples.len(), p90)
        } else {
            format!("The CPU has been heavily loaded: {:.0}% on average, up to {:.0}% in busy moments.", sustained, p90)
        };
        RawFactor::measured(
            score,
            summary,
            vec![
                Measurement::new("Current utilization", format!("{:.0}%", cpu_total)),
                Measurement::new(&format!("Sustained ({} s, excluding background impact)", samples.len()), format!("{:.1}%", sustained)),
                Measurement::new("Busy moments (90th percentile)", format!("{:.1}%", p90)),
                Measurement::new("Counted under Background Process Impact", format!("{:.1}%", bg_cpu)),
            ],
        )
    }

    /// Applies smoothing and weight redistribution.
    fn weigh(&mut self, raw: Vec<(FactorId, RawFactor)>) -> Vec<PressureFactor> {
        let mut smoothed: Vec<(FactorId, RawFactor, Option<f32>)> = Vec::with_capacity(raw.len());
        for (id, factor) in raw {
            let value = match factor.score {
                Some(v) => {
                    let window = self.factor_windows.entry(id).or_default();
                    window.push_back(v);
                    while window.len() > id.smoothing_window_secs() {
                        window.pop_front();
                    }
                    mean(window.iter().copied())
                }
                None => {
                    self.factor_windows.remove(&id);
                    None
                }
            };
            smoothed.push((id, factor, value));
        }

        let available: f32 = smoothed.iter().filter(|(_, _, s)| s.is_some()).map(|(id, _, _)| id.weight()).sum();

        smoothed
            .into_iter()
            .map(|(id, factor, score)| {
                let effective_weight = if score.is_some() && available > 0.0 { id.weight() / available } else { 0.0 };
                let score = score.map(|s| (s * 10.0).round() / 10.0);
                PressureFactor {
                    id,
                    label: id.label().to_string(),
                    weight: id.weight(),
                    effective_weight,
                    score,
                    contribution: effective_weight * score.unwrap_or(0.0),
                    status: pressure::status_for(score),
                    summary: factor.summary,
                    unavailable_reason: factor.unavailable_reason,
                    measurements: factor.measurements,
                }
            })
            .collect()
    }

    fn trend(&mut self, now_ms: u64, ssi: f32) -> Trend {
        self.score_history.push_back((now_ms, ssi));
        while self.score_history.front().is_some_and(|(t, _)| now_ms.saturating_sub(*t) > 130_000) {
            self.score_history.pop_front();
        }

        let oldest_age = self
            .score_history
            .front()
            .map(|(t, _)| now_ms.saturating_sub(*t) / 1000)
            .unwrap_or(0);
        if oldest_age < 45 {
            return Trend { direction: TrendDirection::Stable, delta: 0.0, window_secs: TREND_WINDOW_SECS as u32, has_baseline: false };
        }

        let current = mean(self.score_history.iter().rev().take(5).map(|(_, s)| *s)).unwrap_or(ssi);
        let reference: Vec<f32> = self
            .score_history
            .iter()
            .filter(|(t, _)| {
                let age = now_ms.saturating_sub(*t) / 1000;
                (TREND_WINDOW_SECS - 5..=TREND_WINDOW_SECS + 5).contains(&age)
            })
            .map(|(_, s)| *s)
            .collect();
        let reference = mean(reference.into_iter())
            .or_else(|| mean(self.score_history.iter().take(5).map(|(_, s)| *s)))
            .unwrap_or(current);

        let delta = current - reference;
        let direction = if delta <= -TREND_THRESHOLD {
            TrendDirection::Improving
        } else if delta >= TREND_THRESHOLD {
            TrendDirection::Worsening
        } else {
            TrendDirection::Stable
        };
        Trend { direction, delta: (delta * 10.0).round() / 10.0, window_secs: TREND_WINDOW_SECS as u32, has_baseline: true }
    }

    // -- events ------------------------------------------------------------------------

    fn cooled_down(&mut self, key: &str, now_ms: u64, cooldown_ms: u64) -> bool {
        match self.cooldowns.get(key) {
            Some(last) if now_ms.saturating_sub(*last) < cooldown_ms => false,
            _ => {
                self.cooldowns.insert(key.to_string(), now_ms);
                true
            }
        }
    }

    fn record_events(
        &mut self,
        now_ms: u64,
        level: SsiLevel,
        score: u8,
        factors: &[PressureFactor],
        top: &[TopProcess],
        thermal: &ThermalStatus,
    ) {
        let driver = factors
            .iter()
            .max_by(|a, b| a.contribution.total_cmp(&b.contribution))
            .map(|f| f.label.clone())
            .unwrap_or_default();

        if !self.announced_start {
            self.announced_start = true;
            events::record(
                "analysis",
                "System performance analysis completed",
                &format!("Strain index {} ({}). Largest contributor: {}.", score, level.label(), driver),
                EventStatus::Success,
            );
        }

        if let Some((from, to)) = self.level.observe(level) {
            let rising = rank(to) > rank(from);
            let status = match (rising, to) {
                (true, SsiLevel::Critical) => EventStatus::Critical,
                (true, SsiLevel::High) => EventStatus::Warning,
                (true, _) => EventStatus::Info,
                (false, _) => EventStatus::Success,
            };
            events::record(
                "strain",
                &format!("System strain {} to {}", if rising { "rose" } else { "eased" }, to.label()),
                &format!("Strain index is {}. Largest contributor: {}.", score, driver),
                status,
            );
        }

        let throttling = thermal.throttling == Some(true);
        if throttling && !self.was_throttling && self.cooled_down("thermal", now_ms, 5 * 60 * 1000) {
            events::record(
                "thermal",
                "Thermal throttling detected",
                "Hardware is reducing its speed to stay within temperature limits.",
                EventStatus::Critical,
            );
        }
        self.was_throttling = throttling;

        for p in top.iter().filter(|p| p.importance != Importance::Essential || p.cpu_percent >= 60.0) {
            if p.cpu_percent >= 40.0 && self.cooled_down(&format!("cpu:{}", p.executable), now_ms, EVENT_COOLDOWN_MS) {
                events::record(
                    "high-usage",
                    &format!("High CPU usage detected: {}", p.name),
                    &format!("Using about {:.0}% of the CPU.", p.cpu_percent),
                    EventStatus::Warning,
                );
            }
            if p.memory_percent >= 25.0 && self.cooled_down(&format!("mem:{}", p.executable), now_ms, EVENT_COOLDOWN_MS) {
                events::record(
                    "high-usage",
                    &format!("High memory usage detected: {}", p.name),
                    &format!("Holding {:.0}% of installed memory.", p.memory_percent),
                    EventStatus::Warning,
                );
            }
        }
    }
}

fn rank(level: SsiLevel) -> u8 {
    match level {
        SsiLevel::Minimal => 0,
        SsiLevel::Low => 1,
        SsiLevel::Moderate => 2,
        SsiLevel::High => 3,
        SsiLevel::Critical => 4,
    }
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

// ---------------------------------------------------------------------------------------
// Individual factors
// ---------------------------------------------------------------------------------------

fn memory_factor(m: &SystemMetrics, extras: &ExtraTelemetry, essential_pct: f32, bg_pct: f32) -> RawFactor {
    if m.memory_total_bytes == 0 {
        return RawFactor::unavailable("Windows did not report memory usage.", vec![]);
    }
    let score = pressure::memory_pressure(essential_pct, extras.commit_percent, extras.pages_input_per_sec);
    let available = m.memory_total_bytes.saturating_sub(m.memory_used_bytes);
    let summary = if score < 20.0 {
        format!("{} of {} is free. Memory is not a constraint.", gb(available), gb(m.memory_total_bytes))
    } else if bg_pct >= 5.0 {
        format!(
            "{:.0}% of memory is in use. About {:.0} points of that belong to background programs and are scored separately.",
            m.memory_usage_percent, bg_pct
        )
    } else {
        format!("{:.0}% of memory is in use, leaving {} free.", m.memory_usage_percent, gb(available))
    };
    RawFactor::measured(
        score,
        summary,
        vec![
            Measurement::new("In use", format!("{} of {} ({:.0}%)", gb(m.memory_used_bytes), gb(m.memory_total_bytes), m.memory_usage_percent)),
            Measurement::new("Available", gb(available)),
            Measurement::new(
                "Committed (of commit limit)",
                extras.commit_percent.map_or("Not available".into(), |c| format!("{:.0}%", c)),
            ),
            Measurement::new(
                "Hard page faults",
                extras.pages_input_per_sec.map_or("Not available".into(), |p| format!("{:.0} pages/s", p)),
            ),
            Measurement::new("Counted under Background Process Impact", format!("{:.1}% of memory", bg_pct)),
        ],
    )
}

fn gpu_factor(m: &SystemMetrics, extras: &ExtraTelemetry) -> RawFactor {
    let Some(util) = m.gpu_usage_percent else {
        return RawFactor::unavailable("Windows does not report GPU counters on this PC.", vec![]);
    };
    let vram_pct = extras
        .gpu_memory
        .as_ref()
        .filter(|g| g.total_bytes > 0)
        .map(|g| g.used_bytes as f32 / g.total_bytes as f32 * 100.0);
    let score = pressure::gpu_pressure(util, vram_pct);
    let summary = if score < 20.0 {
        format!("The GPU is mostly idle at {:.0}%.", util)
    } else if score < 55.0 {
        format!("The GPU is working at {:.0}% utilization.", util)
    } else {
        format!("The GPU is heavily loaded at {:.0}% utilization.", util)
    };
    RawFactor::measured(
        score,
        summary,
        vec![
            Measurement::new("Utilization (3D engine)", format!("{:.0}%", util)),
            Measurement::new(
                "Video memory",
                extras.gpu_memory.as_ref().map_or("Not reported by this GPU".into(), |g| {
                    format!("{} of {}", gb(g.used_bytes), gb(g.total_bytes))
                }),
            ),
        ],
    )
}

fn disk_factor(extras: &ExtraTelemetry, avoidable_share: f32) -> RawFactor {
    let Some(active) = extras.disk_active_percent else {
        return RawFactor::unavailable("Windows disk counters are not available yet.", vec![]);
    };
    let essential_active = active * (1.0 - avoidable_share.clamp(0.0, 0.9));
    let score = pressure::disk_pressure(essential_active, extras.disk_latency_ms, extras.disk_queue_length);
    let summary = if score < 20.0 {
        "The disk is responding quickly.".to_string()
    } else if score < 55.0 {
        format!("The disk is busy {:.0}% of the time.", active)
    } else {
        format!("The disk is saturated ({:.0}% busy) and responses are slow.", active)
    };
    RawFactor::measured(
        score,
        summary,
        vec![
            Measurement::new("Busy time", format!("{:.0}%", active)),
            Measurement::new(
                "Response time",
                extras.disk_latency_ms.map_or("Not available".into(), |l| format!("{:.1} ms per transfer", l)),
            ),
            Measurement::new(
                "Queue length",
                extras.disk_queue_length.map_or("Not available".into(), |q| format!("{:.1}", q)),
            ),
            Measurement::new("Counted under Background Process Impact", format!("{:.0}% of disk traffic", avoidable_share * 100.0)),
        ],
    )
}

fn background_factor(bg: &BackgroundAnalysis, bg_cpu: f32, bg_ram_pct: f32, total_ram: u64) -> RawFactor {
    let disk_mb_s = bg.avoidable_disk_bytes_per_sec as f32 / (1024.0 * 1024.0);
    let score = pressure::background_pressure(bg_cpu, bg_ram_pct, disk_mb_s);
    let names: Vec<&str> = bg.groups.iter().take(3).map(|g| g.name.as_str()).collect();
    let summary = if score < 20.0 {
        "Background programs are using very little.".to_string()
    } else {
        format!(
            "Optional background programs use about {:.0}% CPU and {}{}.",
            bg_cpu,
            gb((bg_ram_pct / 100.0 * total_ram as f32) as u64),
            if names.is_empty() { String::new() } else { format!(", mainly {}", names.join(", ")) }
        )
    };
    RawFactor::measured(
        score,
        summary,
        vec![
            Measurement::new("Optional CPU use", format!("{:.1}%", bg_cpu)),
            Measurement::new("Optional memory", format!("{} ({:.1}%)", gb((bg_ram_pct / 100.0 * total_ram as f32) as u64), bg_ram_pct)),
            Measurement::new("Optional disk traffic", format!("{:.1} MB/s", disk_mb_s)),
            Measurement::new("High-impact programs", bg.high_impact_process_count.to_string()),
            Measurement::new("Essential Windows components", "Excluded"),
        ],
    )
}

fn sensor_state(state: &SensorState, margins: pressure::ThermalMargins) -> ThermalSensor {
    ThermalSensor {
        celsius: state.celsius,
        source: state.source.clone(),
        warn_c: margins.warn_c(state.limit_c),
        critical_c: margins.critical_c(state.limit_c),
        limit_c: state.limit_c,
        limit_source: state.limit_source.clone(),
        throttling: state.throttling,
    }
}

fn thermal_factor(extras: &ExtraTelemetry) -> (RawFactor, ThermalStatus) {
    let cpu = sensor_state(&extras.cpu, CPU_MARGINS);
    let gpu = sensor_state(&extras.gpu, GPU_MARGINS);

    let cpu_p = cpu.celsius.map(|c| pressure::thermal_pressure(c, cpu.limit_c, CPU_MARGINS));
    let gpu_p = gpu.celsius.map(|c| pressure::thermal_pressure(c, gpu.limit_c, GPU_MARGINS));
    let throttling = match (cpu.throttling, gpu.throttling) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(false) || b.unwrap_or(false)),
    };

    let hottest = [cpu_p, gpu_p].into_iter().flatten().fold(None, |acc: Option<f32>, v| Some(acc.map_or(v, |a| a.max(v))));
    let state = if throttling == Some(true) {
        ThermalState::Throttling
    } else {
        match hottest {
            None => ThermalState::Unavailable,
            Some(p) if p < 10.0 => ThermalState::Cool,
            Some(p) if p < 25.0 => ThermalState::Normal,
            Some(p) if p < 75.0 => ThermalState::Warm,
            Some(_) => ThermalState::Hot,
        }
    };

    let describe = |s: &ThermalSensor| match s.celsius {
        Some(c) => format!("{:.0} °C (limit {:.0} °C, {})", c, s.limit_c, if s.limit_source == "device" { "read from device" } else { "typical" }),
        None => "Sensor unavailable".to_string(),
    };
    let measurements = vec![
        Measurement::new("CPU temperature", describe(&cpu)),
        Measurement::new("GPU temperature", describe(&gpu)),
        Measurement::new(
            "Thermal throttling",
            match throttling {
                Some(true) => "Active",
                Some(false) => "None detected",
                None => "Cannot be detected on this PC",
            },
        ),
    ];

    let raw = match (hottest, throttling) {
        (None, Some(true)) => RawFactor::measured(
            pressure::THROTTLING_FLOOR,
            "Hardware reports thermal throttling, but no temperature sensor is readable.".into(),
            measurements,
        ),
        (None, _) => RawFactor::unavailable("No temperature sensor is exposed by this PC.", measurements),
        (Some(p), t) => {
            let score = if t == Some(true) { p.max(pressure::THROTTLING_FLOOR) } else { p };
            let summary = match state {
                ThermalState::Throttling => "Hardware is slowing itself down to avoid overheating.".to_string(),
                ThermalState::Hot => "Temperatures are close to the limit of this hardware.".to_string(),
                ThermalState::Warm => "Temperatures are elevated but within safe limits.".to_string(),
                _ => "Temperatures are comfortable.".to_string(),
            };
            RawFactor::measured(score, summary, measurements)
        }
    };

    (raw, ThermalStatus { cpu, gpu, state, throttling })
}

fn startup_factor(startup: Option<&StartupSummary>) -> RawFactor {
    let Some(s) = startup else {
        return RawFactor::unavailable("Startup items have not been scanned yet.", vec![]);
    };
    let delay_s = s.estimated_delay_ms as f32 / 1000.0;
    let score = pressure::startup_pressure(delay_s, s.weighted_count);
    let summary = if s.enabled_optional == 0 {
        "No optional programs start with Windows.".to_string()
    } else {
        format!("{} optional programs start with Windows, adding about {:.1} s.", s.enabled_optional, delay_s)
    };
    RawFactor::measured(
        score,
        summary,
        vec![
            Measurement::new("Enabled startup applications", s.enabled_optional.to_string()),
            Measurement::new("High-impact items", s.high_impact.to_string()),
            Measurement::new("Estimated startup delay", format!("{:.1} s", delay_s)),
            Measurement::new("All startup entries scanned", s.total_items.to_string()),
        ],
    )
}

fn storage_factor(drives: &[DriveInfo]) -> RawFactor {
    let Some(d) = drives.iter().find(|d| d.is_system).or_else(|| drives.first()) else {
        return RawFactor::unavailable("No local drive could be read.", vec![]);
    };
    let score = pressure::storage_pressure(d.used_percent, d.free_bytes);
    let summary = if score < 20.0 {
        format!("Drive {} has {} free.", d.drive, gb(d.free_bytes))
    } else {
        format!("Drive {} is {:.0}% full with {} free.", d.drive, d.used_percent, gb(d.free_bytes))
    };
    RawFactor::measured(
        score,
        summary,
        vec![
            Measurement::new("Drive", format!("{} (system drive)", d.drive)),
            Measurement::new("Used", format!("{} of {} ({:.0}%)", gb(d.used_bytes), gb(d.total_bytes), d.used_percent)),
            Measurement::new("Free", gb(d.free_bytes)),
        ],
    )
}

fn boot_factor(boot: &BootState) -> RawFactor {
    match boot {
        BootState::Pending => RawFactor::unavailable("Reading Windows boot history.", vec![]),
        BootState::Unavailable(reason) => RawFactor::unavailable(reason, vec![]),
        BootState::Available { latest_ms, baseline_ms, samples } => {
            let latest = *latest_ms as f32 / 1000.0;
            let baseline = baseline_ms.map(|b| b as f32 / 1000.0);
            let score = pressure::boot_pressure(latest, baseline);
            let summary = match baseline {
                Some(b) if latest > b * 1.3 => format!("The last boot took {:.0} s, slower than the usual {:.0} s.", latest, b),
                Some(b) => format!("The last boot took {:.0} s, in line with the usual {:.0} s.", latest, b),
                None => format!("The last boot took {:.0} s. More boots are needed for a comparison.", latest),
            };
            RawFactor::measured(
                score,
                summary,
                vec![
                    Measurement::new("Last boot duration", format!("{:.0} s", latest)),
                    Measurement::new("Typical boot (median of earlier boots)", baseline.map_or("Not enough history".into(), |b| format!("{:.0} s", b))),
                    Measurement::new("Boots recorded", samples.to_string()),
                ],
            )
        }
    }
}

// ---------------------------------------------------------------------------------------
// Summary, performance card, top processes
// ---------------------------------------------------------------------------------------

fn ssi_summary(level: SsiLevel, factors: &[PressureFactor]) -> String {
    let lead = match level {
        SsiLevel::Minimal => "Your system is running comfortably.",
        SsiLevel::Low => "Your system is running well under a light load.",
        SsiLevel::Moderate => "Your system is under moderate strain.",
        SsiLevel::High => "Your system is under heavy strain.",
        SsiLevel::Critical => "Your system is severely strained.",
    };
    let mut ranked: Vec<&PressureFactor> = factors.iter().filter(|f| f.contribution >= 4.0).collect();
    ranked.sort_by(|a, b| b.contribution.total_cmp(&a.contribution));
    let drivers: Vec<String> = ranked.iter().take(2).map(|f| f.label.to_lowercase()).collect();
    match drivers.as_slice() {
        [] => lead.to_string(),
        [a] => format!("{lead} The main driver is {a}."),
        [a, b] => format!("{lead} The main drivers are {a} and {b}."),
        _ => lead.to_string(),
    }
}

fn performance_impact(
    factors: &[PressureFactor],
    bg: &BackgroundAnalysis,
    startup: Option<&StartupSummary>,
    ssi: f32,
    bg_cpu: f32,
    bg_ram_pct: f32,
    total_ram: u64,
) -> PerformanceImpact {
    let score_of = |id: FactorId| factors.iter().find(|f| f.id == id).and_then(|f| f.score);

    let mut bottlenecks: Vec<Bottleneck> = factors
        .iter()
        .filter(|f| f.score.unwrap_or(0.0) >= 35.0 && f.contribution > 0.0)
        .map(|f| Bottleneck {
            factor: f.id,
            label: f.label.clone(),
            score: f.score.unwrap_or(0.0),
            contribution: round1(f.contribution),
            share_percent: if ssi > 0.0 { round1(f.contribution / ssi * 100.0) } else { 0.0 },
            detail: f.summary.clone(),
        })
        .collect();
    bottlenecks.sort_by(|a, b| b.contribution.total_cmp(&a.contribution));
    bottlenecks.truncate(3);

    PerformanceImpact {
        startup: StartupImpactInfo {
            available: startup.is_some(),
            score: score_of(FactorId::Startup),
            enabled_count: startup.map_or(0, |s| s.enabled_optional),
            total_count: startup.map_or(0, |s| s.total_items),
            high_impact_count: startup.map_or(0, |s| s.high_impact),
            estimated_delay_ms: startup.map_or(0, |s| s.estimated_delay_ms),
            top_items: startup.map(|s| s.top_items.clone()).unwrap_or_default(),
        },
        background: BackgroundImpactInfo {
            available: true,
            score: score_of(FactorId::Background),
            high_impact_process_count: bg.high_impact_process_count,
            avoidable_cpu_percent: round1(bg_cpu),
            avoidable_memory_bytes: (bg_ram_pct / 100.0 * total_ram as f32) as u64,
            avoidable_memory_percent: round1(bg_ram_pct),
        },
        bottlenecks,
    }
}

struct Group {
    name: String,
    executable: String,
    category: ProcessCategory,
    instances: usize,
    primary_pid: u32,
    primary_load: f32,
    cpu: f32,
    memory: u64,
    gpu: f32,
    essential: bool,
    foreground: bool,
    review: bool,
}

pub fn top_processes(processes: &[ProcessInfo], extras: &ExtraTelemetry, total_ram: u64) -> Vec<TopProcess> {
    const LIMIT: usize = 5;
    let gpu_known = !extras.gpu_per_pid.is_empty();
    let mut groups: HashMap<String, Group> = HashMap::new();

    for p in processes {
        let lower = p.name.to_lowercase();
        if p.pid == 0 || lower == "system idle process" || lower == "idle" {
            continue;
        }
        let load = p.sustained_cpu_percent + p.memory_bytes as f32 / (1024.0 * 1024.0 * 1024.0);
        let g = groups.entry(lower).or_insert_with(|| Group {
            name: display_name(p),
            executable: p.name.clone(),
            category: p.category,
            instances: 0,
            primary_pid: p.pid,
            primary_load: -1.0,
            cpu: 0.0,
            memory: 0,
            gpu: 0.0,
            essential: true,
            foreground: false,
            review: false,
        });
        g.instances += 1;
        g.cpu += p.sustained_cpu_percent;
        g.memory = g.memory.saturating_add(p.memory_bytes);
        g.gpu += extras.gpu_per_pid.get(&p.pid).copied().unwrap_or(0.0);
        if load > g.primary_load {
            g.primary_load = load;
            g.primary_pid = p.pid;
        }
        let essential_here = p.is_system_critical || matches!(p.category, ProcessCategory::WindowsCore | ProcessCategory::Drivers);
        g.essential &= essential_here;
        g.foreground |= p.activity_state == ProcessActivityState::Foreground;
        g.review |= p.activity_state != ProcessActivityState::Foreground
            && !essential_here
            && (p.impact_level == crate::analyzer::LoadImpactLevel::High
                || matches!(p.status, ProcessStatus::Attention | ProcessStatus::HighResource));
    }

    let mut out: Vec<TopProcess> = groups
        .into_values()
        .map(|g| {
            let memory_percent = if total_ram > 0 { g.memory as f32 / total_ram as f32 * 100.0 } else { 0.0 };
            let gpu = if gpu_known { Some(g.gpu.clamp(0.0, 100.0)) } else { None };
            let load_score = g.cpu * 0.5 + memory_percent * 0.35 + gpu.unwrap_or(0.0) * 0.15;
            let (importance, classification) = if g.review {
                (Importance::Review, "Background · worth reviewing")
            } else if g.foreground {
                (Importance::Active, "Active application")
            } else if g.essential {
                (Importance::Essential, "Windows component")
            } else {
                (Importance::Background, "Background application")
            };
            TopProcess {
                name: g.name,
                executable: g.executable,
                category: category_label(g.category).to_string(),
                instances: g.instances,
                cpu_percent: round1(g.cpu),
                memory_bytes: g.memory,
                memory_percent: round1(memory_percent),
                gpu_percent: gpu.map(round1),
                importance,
                classification: classification.to_string(),
                load_score: round1(load_score),
                cpu_high: g.cpu >= 25.0,
                memory_high: memory_percent >= 10.0 || g.memory >= 1200 * 1024 * 1024,
                primary_pid: g.primary_pid,
            }
        })
        .collect();

    out.sort_by(|a, b| b.load_score.total_cmp(&a.load_score));
    out.truncate(LIMIT);
    out
}

fn category_label(c: ProcessCategory) -> &'static str {
    match c {
        ProcessCategory::WindowsCore => "windows",
        ProcessCategory::Drivers => "driver",
        ProcessCategory::Gaming => "gaming",
        ProcessCategory::Development => "development",
        ProcessCategory::Productivity => "productivity",
        ProcessCategory::Communication => "communication",
        ProcessCategory::Browser => "browser",
        ProcessCategory::Unknown => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::SystemMetrics;
    use crate::collector::extras::ExtraTelemetry;

    fn metrics(cpu: f32, mem: f32) -> SystemMetrics {
        SystemMetrics {
            cpu_usage_percent: cpu,
            cpu_delta_percent: 0.0,
            memory_usage_percent: mem,
            memory_used_bytes: (16.0 * 1024.0 * 1024.0 * 1024.0 * mem as f64 / 100.0) as u64,
            memory_total_bytes: 16 * 1024 * 1024 * 1024,
            memory_delta_percent: 0.0,
            gpu_usage_percent: None,
            gpu_delta_percent: None,
            disk_bytes_per_sec: 0,
            network_bytes_per_sec: None,
            network_download_bytes_per_sec: None,
            network_upload_bytes_per_sec: None,
            foreground_load_percent: 0.0,
            background_load_percent: 0.0,
            system_strain_percent: 0.0,
            user_active: true,
            foreground_process_name: None,
            total_processes: 0,
            attention_processes: 0,
            protected_processes: 0,
            high_resource_processes: 0,
            system_status: "healthy".into(),
            timestamp_ms: 0,
        }
    }

    fn drive(used_percent: f32, free_gb: u64) -> DriveInfo {
        let free = free_gb * 1024 * 1024 * 1024;
        DriveInfo {
            drive: "C:".into(),
            name: "Windows".into(),
            total_bytes: free * 100 / (100.0 - used_percent).max(1.0) as u64,
            used_bytes: 0,
            free_bytes: free,
            used_percent,
            is_system: true,
        }
    }

    fn run(engine: &mut SsiEngine, m: &SystemMetrics, extras: &ExtraTelemetry, drives: &[DriveInfo], t: u64) -> DashboardSnapshot {
        engine.update(&SsiInputs {
            now_ms: t,
            metrics: m,
            processes: &[],
            extras,
            startup: None,
            drives,
            composition: None,
            boot: &BootState::Unavailable("test".into()),
            device_name: "test",
            uptime_seconds: 10,
        })
    }

    #[test]
    fn unavailable_factors_hand_their_weight_to_the_others() {
        let mut engine = SsiEngine::new();
        let snap = run(&mut engine, &metrics(30.0, 50.0), &ExtraTelemetry::default(), &[drive(50.0, 200)], 1_000);
        let f = &snap.ssi.factors;
        let used: f32 = f.iter().map(|x| x.effective_weight).sum();
        assert!((used - 1.0).abs() < 1e-4, "effective weights sum to {used}");
        for id in [FactorId::Gpu, FactorId::DiskIo, FactorId::Thermal, FactorId::Startup, FactorId::Boot] {
            let factor = f.iter().find(|x| x.id == id).unwrap();
            assert_eq!(factor.status, Status::Unavailable, "{id:?}");
            assert_eq!(factor.effective_weight, 0.0);
            assert_eq!(factor.contribution, 0.0);
        }
        let available: f32 = f.iter().filter(|x| x.score.is_some()).map(|x| x.weight).sum();
        assert!((snap.ssi.available_weight - available).abs() < 1e-5);
    }

    #[test]
    fn score_is_the_weighted_sum_of_the_factors() {
        let mut engine = SsiEngine::new();
        let snap = run(&mut engine, &metrics(80.0, 90.0), &ExtraTelemetry::default(), &[drive(92.0, 20)], 1_000);
        let sum: f32 = snap.ssi.factors.iter().map(|f| f.contribution).sum();
        assert_eq!(snap.ssi.score, sum.round() as u8);
        assert!(snap.ssi.score > 40, "a loaded machine should score above 40, got {}", snap.ssi.score);
        assert_eq!(snap.ssi.level, SsiLevel::from_score(snap.ssi.score));
    }

    #[test]
    fn loaded_machine_scores_higher_than_an_idle_one() {
        let drives = [drive(50.0, 200)];
        let mut idle = SsiEngine::new();
        let mut busy = SsiEngine::new();
        let mut idle_score = 0;
        let mut busy_score = 0;
        for i in 0..40 {
            idle_score = run(&mut idle, &metrics(3.0, 40.0), &ExtraTelemetry::default(), &drives, 1_000 * (i + 1)).ssi.score;
            busy_score = run(&mut busy, &metrics(95.0, 92.0), &ExtraTelemetry::default(), &drives, 1_000 * (i + 1)).ssi.score;
        }
        assert!(idle_score <= 20, "idle machine scored {idle_score}");
        assert!(busy_score > idle_score + 25, "busy {busy_score} vs idle {idle_score}");
    }

    #[test]
    fn a_brief_spike_does_not_move_the_score_much() {
        let drives = [drive(50.0, 200)];
        let mut engine = SsiEngine::new();
        let mut before = 0;
        for i in 0..40 {
            before = run(&mut engine, &metrics(5.0, 40.0), &ExtraTelemetry::default(), &drives, 1_000 * (i + 1)).ssi.score;
        }
        let during = run(&mut engine, &metrics(100.0, 40.0), &ExtraTelemetry::default(), &drives, 41_000).ssi.score;
        assert!(during.saturating_sub(before) <= 8, "one second of 100% CPU moved the score {before} -> {during}");
    }

    #[test]
    fn missing_temperature_sensors_are_reported_not_invented() {
        let mut engine = SsiEngine::new();
        let snap = run(&mut engine, &metrics(10.0, 30.0), &ExtraTelemetry::default(), &[drive(40.0, 300)], 1_000);
        assert_eq!(snap.thermal.state, ThermalState::Unavailable);
        assert!(snap.thermal.cpu.celsius.is_none());
        assert!(snap.thermal.gpu.celsius.is_none());
    }

    #[test]
    fn throttling_forces_high_thermal_pressure() {
        let mut extras = ExtraTelemetry::default();
        extras.cpu.celsius = Some(60.0);
        extras.cpu.throttling = Some(true);
        let mut engine = SsiEngine::new();
        let snap = run(&mut engine, &metrics(10.0, 30.0), &extras, &[drive(40.0, 300)], 1_000);
        let thermal = snap.ssi.factors.iter().find(|f| f.id == FactorId::Thermal).unwrap();
        assert!(thermal.score.unwrap() >= pressure::THROTTLING_FLOOR);
        assert_eq!(snap.thermal.state, ThermalState::Throttling);
    }

    #[test]
    fn level_changes_need_to_hold_before_they_are_reported() {
        let mut t = LevelTracker::default();
        assert_eq!(t.observe(SsiLevel::Low), None);
        for _ in 0..(LEVEL_CONFIRM_SECS - 1) {
            assert_eq!(t.observe(SsiLevel::High), None);
        }
        assert_eq!(t.observe(SsiLevel::High), Some((SsiLevel::Low, SsiLevel::High)));
        // A flicker back and forth never confirms.
        let mut t = LevelTracker::default();
        t.observe(SsiLevel::Low);
        for _ in 0..30 {
            assert_eq!(t.observe(SsiLevel::High), None.or(t.observe(SsiLevel::Low)));
        }
    }
}
