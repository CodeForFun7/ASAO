//! Normalisation of raw measurements into 0..100 pressure scores.
//!
//! Every function here is pure: numbers in, a score out. A score of 0 means no pressure and
//! 100 means the resource is exhausted. The curves are piecewise linear so each breakpoint can
//! be read as "at this reading the pressure is that high".

use super::model::{SsiLevel, Status};

/// Linear interpolation through `(input, score)` breakpoints sorted by input.
/// Inputs outside the table clamp to the first or last score.
pub fn piecewise(x: f32, points: &[(f32, f32)]) -> f32 {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return 0.0;
    };
    if !x.is_finite() || x <= first.0 {
        return first.1.clamp(0.0, 100.0);
    }
    if x >= last.0 {
        return last.1.clamp(0.0, 100.0);
    }
    for pair in points.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        if x <= x1 {
            let t = (x - x0) / (x1 - x0);
            return (y0 + (y1 - y0) * t).clamp(0.0, 100.0);
        }
    }
    last.1.clamp(0.0, 100.0)
}

pub fn clamp100(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 100.0) } else { 0.0 }
}

const CPU_CURVE: &[(f32, f32)] =
    &[(0.0, 0.0), (15.0, 5.0), (35.0, 20.0), (55.0, 42.0), (75.0, 70.0), (90.0, 92.0), (100.0, 100.0)];

/// CPU pressure from sustained use and the busiest moments inside the window.
/// Sustained load counts most; a high 90th percentile adds to it.
pub fn cpu_pressure(sustained_percent: f32, p90_percent: f32) -> f32 {
    clamp100(piecewise(sustained_percent, CPU_CURVE) * 0.7 + piecewise(p90_percent, CPU_CURVE) * 0.3)
}

const MEMORY_USED_CURVE: &[(f32, f32)] =
    &[(0.0, 0.0), (50.0, 0.0), (65.0, 15.0), (80.0, 45.0), (90.0, 75.0), (97.0, 100.0)];
const COMMIT_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (60.0, 0.0), (80.0, 45.0), (95.0, 100.0)];
const PAGING_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (50.0, 5.0), (200.0, 40.0), (800.0, 80.0), (2000.0, 100.0)];

/// Memory pressure from how little physical memory is left, how close committed memory is to
/// its limit, and how often Windows has to read pages back from disk.
pub fn memory_pressure(used_percent: f32, commit_percent: Option<f32>, pages_input_per_sec: Option<f32>) -> f32 {
    let mut weights = 0.65;
    let mut total = piecewise(used_percent, MEMORY_USED_CURVE) * 0.65;
    if let Some(c) = commit_percent {
        total += piecewise(c, COMMIT_CURVE) * 0.20;
        weights += 0.20;
    }
    if let Some(p) = pages_input_per_sec {
        total += piecewise(p, PAGING_CURVE) * 0.15;
        weights += 0.15;
    }
    clamp100(total / weights)
}

const GPU_UTIL_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (30.0, 10.0), (60.0, 35.0), (80.0, 65.0), (95.0, 95.0), (100.0, 100.0)];
const VRAM_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (50.0, 10.0), (75.0, 40.0), (90.0, 75.0), (98.0, 100.0)];

/// GPU pressure from engine utilisation and, when known, video memory use.
pub fn gpu_pressure(util_percent: f32, vram_percent: Option<f32>) -> f32 {
    let util = piecewise(util_percent, GPU_UTIL_CURVE);
    match vram_percent {
        Some(v) => clamp100(util * 0.7 + piecewise(v, VRAM_CURVE) * 0.3),
        None => clamp100(util),
    }
}

const DISK_ACTIVE_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (20.0, 10.0), (50.0, 35.0), (80.0, 70.0), (100.0, 95.0)];
const DISK_LATENCY_CURVE: &[(f32, f32)] =
    &[(0.0, 0.0), (5.0, 10.0), (15.0, 35.0), (30.0, 60.0), (60.0, 85.0), (120.0, 100.0)];
const DISK_QUEUE_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (1.0, 20.0), (2.0, 50.0), (4.0, 100.0)];

/// Disk pressure from how busy the disks are and how long a transfer takes.
/// Latency of an idle disk is noise, so it only counts once the disk is doing real work.
pub fn disk_pressure(active_percent: f32, latency_ms: Option<f32>, queue_length: Option<f32>) -> f32 {
    let active = piecewise(active_percent, DISK_ACTIVE_CURVE);
    let busy_enough = active_percent >= 5.0;
    let mut total = active * 0.4;
    let mut weights = 0.4;
    if let (Some(l), true) = (latency_ms, busy_enough) {
        total += piecewise(l, DISK_LATENCY_CURVE) * 0.4;
        weights += 0.4;
    }
    if let (Some(q), true) = (queue_length, busy_enough) {
        total += piecewise(q, DISK_QUEUE_CURVE) * 0.2;
        weights += 0.2;
    }
    clamp100(total / weights)
}

/// Degrees below the device limit at which pressure reaches 10, 25, 45, 75 and 100.
#[derive(Clone, Copy)]
pub struct ThermalMargins([f32; 5]);

pub const CPU_MARGINS: ThermalMargins = ThermalMargins([40.0, 30.0, 20.0, 10.0, 0.0]);
pub const GPU_MARGINS: ThermalMargins = ThermalMargins([38.0, 28.0, 18.0, 8.0, 0.0]);

impl ThermalMargins {
    /// Temperature at which the part counts as warm (pressure 45).
    pub fn warn_c(self, limit_c: f32) -> f32 {
        limit_c - self.0[2]
    }
    /// Temperature at which the part counts as hot (pressure 75).
    pub fn critical_c(self, limit_c: f32) -> f32 {
        limit_c - self.0[3]
    }
}

/// Pressure of one temperature reading against the limit of that very device.
pub fn thermal_pressure(celsius: f32, limit_c: f32, margins: ThermalMargins) -> f32 {
    let m = margins.0;
    let points = [
        (limit_c - m[0] - 15.0, 0.0),
        (limit_c - m[0], 10.0),
        (limit_c - m[1], 25.0),
        (limit_c - m[2], 45.0),
        (limit_c - m[3], 75.0),
        (limit_c - m[4], 100.0),
    ];
    piecewise(celsius, &points)
}

/// Pressure never reads below this while the hardware reports active thermal throttling.
pub const THROTTLING_FLOOR: f32 = 85.0;

const STARTUP_DELAY_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (3.0, 15.0), (8.0, 40.0), (15.0, 70.0), (30.0, 100.0)];
const STARTUP_COUNT_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (5.0, 10.0), (12.0, 35.0), (25.0, 70.0), (45.0, 100.0)];

/// Startup pressure from the estimated sign-in delay and the weighted number of enabled items
/// (low impact counts 1, medium 2.5, high 5).
pub fn startup_pressure(estimated_delay_secs: f32, weighted_item_count: f32) -> f32 {
    clamp100(
        piecewise(estimated_delay_secs, STARTUP_DELAY_CURVE) * 0.6
            + piecewise(weighted_item_count, STARTUP_COUNT_CURVE) * 0.4,
    )
}

const USED_SPACE_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (70.0, 5.0), (80.0, 20.0), (90.0, 55.0), (95.0, 80.0), (99.0, 100.0)];
const FREE_GB_CURVE: &[(f32, f32)] = &[(0.0, 100.0), (1.0, 100.0), (5.0, 80.0), (10.0, 50.0), (20.0, 25.0), (50.0, 0.0)];

/// Storage pressure from the fullness of the drive Windows runs from. Whichever of
/// "percent used" and "absolute free space" looks worse decides, since a 2 TB drive that is
/// 95% full still has room, and a 64 GB drive that is 80% full does not.
pub fn storage_pressure(used_percent: f32, free_bytes: u64) -> f32 {
    let free_gb = free_bytes as f32 / (1024.0 * 1024.0 * 1024.0);
    let by_percent = piecewise(used_percent, USED_SPACE_CURVE);
    let by_free = piecewise(free_gb, FREE_GB_CURVE);
    // A large drive is only in trouble when it is also nearly full.
    clamp100(by_percent.max(by_free.min(by_percent * 2.0 + 10.0)))
}

const BOOT_RATIO_CURVE: &[(f32, f32)] = &[(0.8, 0.0), (1.0, 10.0), (1.3, 35.0), (1.7, 65.0), (2.5, 100.0)];
const BOOT_ABSOLUTE_CURVE: &[(f32, f32)] = &[(30.0, 0.0), (45.0, 20.0), (75.0, 50.0), (120.0, 85.0), (180.0, 100.0)];

/// Boot pressure from the last boot duration, compared with the median of earlier boots when
/// there are at least three, and always against what is acceptable in absolute terms.
pub fn boot_pressure(latest_secs: f32, baseline_secs: Option<f32>) -> f32 {
    let absolute = piecewise(latest_secs, BOOT_ABSOLUTE_CURVE);
    match baseline_secs.filter(|b| *b > 1.0) {
        Some(base) => clamp100(piecewise(latest_secs / base, BOOT_RATIO_CURVE) * 0.6 + absolute * 0.4),
        None => clamp100(absolute),
    }
}

const BG_CPU_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (2.0, 10.0), (5.0, 30.0), (10.0, 55.0), (20.0, 85.0), (35.0, 100.0)];
const BG_RAM_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (5.0, 10.0), (10.0, 30.0), (20.0, 55.0), (30.0, 80.0), (45.0, 100.0)];
const BG_DISK_CURVE: &[(f32, f32)] = &[(0.0, 0.0), (2.0, 15.0), (10.0, 50.0), (30.0, 85.0), (60.0, 100.0)];

/// Background process impact from the *avoidable* share of CPU, memory and disk traffic:
/// resources used by background programs the user is not working in, weighted by how
/// optional each program is. Essential Windows components are not included.
/// The busiest dimension dominates so that one extreme resource is not averaged away.
pub fn background_pressure(avoidable_cpu_percent: f32, avoidable_ram_percent: f32, avoidable_disk_mb_s: f32) -> f32 {
    let cpu = piecewise(avoidable_cpu_percent, BG_CPU_CURVE);
    let ram = piecewise(avoidable_ram_percent, BG_RAM_CURVE);
    let disk = piecewise(avoidable_disk_mb_s, BG_DISK_CURVE);
    let busiest = cpu.max(ram).max(disk);
    let weighted = cpu * 0.45 + ram * 0.40 + disk * 0.15;
    clamp100(busiest * 0.6 + weighted * 0.4)
}

/// Status colour of a single factor.
pub fn status_for(score: Option<f32>) -> Status {
    match score {
        None => Status::Unavailable,
        Some(s) if s < 40.0 => Status::Healthy,
        Some(s) if s < 70.0 => Status::Warning,
        Some(_) => Status::Critical,
    }
}

pub fn level_for(score: u8) -> SsiLevel {
    SsiLevel::from_score(score)
}

/// Mean of the values, or `None` for an empty window.
pub fn mean(values: impl Iterator<Item = f32>) -> Option<f32> {
    let mut sum = 0.0;
    let mut n = 0usize;
    for v in values {
        sum += v;
        n += 1;
    }
    (n > 0).then(|| sum / n as f32)
}

/// 90th percentile of the values (nearest rank), or `None` for an empty window.
pub fn percentile90(values: &[f32]) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let rank = ((sorted.len() as f32) * 0.9).ceil() as usize;
    Some(sorted[rank.clamp(1, sorted.len()) - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piecewise_interpolates_and_clamps() {
        let pts = [(0.0, 0.0), (10.0, 50.0), (20.0, 100.0)];
        assert_eq!(piecewise(-5.0, &pts), 0.0);
        assert_eq!(piecewise(5.0, &pts), 25.0);
        assert_eq!(piecewise(15.0, &pts), 75.0);
        assert_eq!(piecewise(99.0, &pts), 100.0);
        assert_eq!(piecewise(f32::NAN, &pts), 0.0);
    }

    #[test]
    fn pressures_stay_between_0_and_100() {
        for x in [-10.0, 0.0, 33.3, 100.0, 5000.0] {
            for v in [
                cpu_pressure(x, x),
                memory_pressure(x, Some(x), Some(x)),
                gpu_pressure(x, Some(x)),
                disk_pressure(x, Some(x), Some(x)),
                thermal_pressure(x, 100.0, CPU_MARGINS),
                startup_pressure(x, x),
                storage_pressure(x, (x.max(0.0) as u64) * 1_000_000_000),
                boot_pressure(x.abs(), Some(40.0)),
                background_pressure(x, x, x),
            ] {
                assert!((0.0..=100.0).contains(&v), "{v} out of range for input {x}");
            }
        }
    }

    #[test]
    fn an_idle_machine_has_no_pressure() {
        assert_eq!(cpu_pressure(3.0, 6.0) < 5.0, true);
        assert_eq!(memory_pressure(40.0, Some(30.0), Some(0.0)), 0.0);
        assert_eq!(disk_pressure(1.0, Some(0.2), Some(0.0)), 0.0 + piecewise(1.0, DISK_ACTIVE_CURVE));
        assert_eq!(thermal_pressure(40.0, 100.0, CPU_MARGINS), 0.0);
    }

    #[test]
    fn more_load_never_lowers_pressure() {
        let mut last = 0.0;
        for cpu in 0..=100 {
            let p = cpu_pressure(cpu as f32, cpu as f32);
            assert!(p >= last, "cpu pressure fell at {cpu}%");
            last = p;
        }
        let mut last = 0.0;
        for t in 30..=110 {
            let p = thermal_pressure(t as f32, 100.0, CPU_MARGINS);
            assert!(p >= last, "thermal pressure fell at {t} C");
            last = p;
        }
    }

    #[test]
    fn thermal_limit_is_per_device() {
        // The same 80 C reading is far more serious on a part that slows down at 85 C.
        let relaxed = thermal_pressure(80.0, 100.0, CPU_MARGINS);
        let tight = thermal_pressure(80.0, 85.0, CPU_MARGINS);
        assert!(tight > relaxed + 20.0, "{tight} vs {relaxed}");
        assert_eq!(thermal_pressure(100.0, 100.0, CPU_MARGINS), 100.0);
    }

    #[test]
    fn disk_latency_of_an_idle_disk_is_ignored() {
        let idle = disk_pressure(0.5, Some(400.0), Some(9.0));
        let busy = disk_pressure(60.0, Some(400.0), Some(9.0));
        assert!(idle < 5.0);
        assert!(busy > 70.0);
    }

    #[test]
    fn storage_depends_on_both_fullness_and_free_space() {
        const GB: u64 = 1024 * 1024 * 1024;
        // Tiny drive, 80% full, 12 GB left
        let small = storage_pressure(80.0, 12 * GB);
        // Huge drive, 80% full, 400 GB left
        let large = storage_pressure(80.0, 400 * GB);
        assert!(small > large);
        // Nearly full is always serious
        assert!(storage_pressure(98.0, 3 * GB) > 80.0);
        // Plenty of room
        assert!(storage_pressure(40.0, 300 * GB) < 5.0);
    }

    #[test]
    fn boot_compares_against_history() {
        let normal = boot_pressure(40.0, Some(40.0));
        let regressed = boot_pressure(80.0, Some(40.0));
        assert!(regressed > normal + 30.0);
        assert!(boot_pressure(40.0, None) < 25.0);
    }

    #[test]
    fn background_pressure_follows_the_busiest_resource() {
        let none = background_pressure(0.0, 0.0, 0.0);
        let ram_heavy = background_pressure(0.5, 35.0, 0.0);
        assert_eq!(none, 0.0);
        assert!(ram_heavy > 50.0, "{ram_heavy}");
    }

    #[test]
    fn statistics_helpers() {
        assert_eq!(mean([1.0, 2.0, 3.0].into_iter()), Some(2.0));
        assert_eq!(mean(std::iter::empty()), None);
        let v: Vec<f32> = (1..=10).map(|x| x as f32).collect();
        assert_eq!(percentile90(&v), Some(9.0));
        assert_eq!(percentile90(&[]), None);
    }

    #[test]
    fn status_bands() {
        assert_eq!(status_for(None), Status::Unavailable);
        assert_eq!(status_for(Some(39.9)), Status::Healthy);
        assert_eq!(status_for(Some(40.0)), Status::Warning);
        assert_eq!(status_for(Some(70.0)), Status::Critical);
    }
}
