//! Background process analysis for the SSI.
//!
//! "Background impact" measures resources used by programs the user is not working in and could
//! plausibly do without. To avoid counting the same CPU or memory twice, the amount attributed
//! to background impact is *subtracted* from the system-wide CPU, memory and disk readings
//! before those factors are scored. Each resource is therefore scored in exactly one place:
//! essential load in its own factor, avoidable load here.

use std::collections::HashMap;

use crate::analyzer::{LoadImpactLevel, ProcessActivityState, ProcessCategory, ProcessInfo, ProcessStatus};

/// How optional a process is, from 0 (part of Windows, never counted) to 1 (clearly optional).
pub fn optional_weight(p: &ProcessInfo) -> f32 {
    if p.is_system_critical {
        return 0.0;
    }
    let base = match p.category {
        ProcessCategory::WindowsCore | ProcessCategory::Drivers => return 0.0,
        ProcessCategory::Browser => 0.55,
        ProcessCategory::Development => 0.5,
        ProcessCategory::Productivity => 0.6,
        ProcessCategory::Communication => 0.7,
        ProcessCategory::Gaming => 0.9,
        ProcessCategory::Unknown => 1.0,
    };
    // Something Windows launches by itself at sign-in is more likely to be unwanted.
    let startup_bonus = if p.is_startup { 0.1 } else { 0.0 };
    (base + startup_bonus).min(1.0)
}

/// Readable program name: the product name when it is short and specific, else the file name.
pub fn display_name(p: &ProcessInfo) -> String {
    if let Some(product) = p.product_name.as_deref().map(str::trim) {
        let generic = product.contains("Operating System") || product.contains("Windows");
        if !product.is_empty() && product.len() <= 26 && !generic {
            return product.to_string();
        }
    }
    let name = p.name.trim();
    let stem = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name);
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => name.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct BackgroundGroup {
    pub name: String,
    pub instances: usize,
    pub primary_pid: u32,
    /// Sustained CPU of all instances, in percent of the whole CPU.
    pub cpu: f32,
    pub memory_bytes: u64,
    pub weighted_cpu: f32,
    pub weighted_memory_bytes: u64,
    pub high_impact: bool,
}

#[derive(Debug, Clone, Default)]
pub struct BackgroundAnalysis {
    /// Optional CPU use of background programs, in percent of the whole CPU.
    pub avoidable_cpu_percent: f32,
    pub avoidable_memory_bytes: u64,
    pub avoidable_disk_bytes_per_sec: u64,
    /// Share of all process disk traffic that is avoidable, 0..1.
    pub avoidable_disk_share: f32,
    pub high_impact_process_count: usize,
    /// Background programs, heaviest first.
    pub groups: Vec<BackgroundGroup>,
}

pub fn analyze(processes: &[ProcessInfo]) -> BackgroundAnalysis {
    let mut cpu = 0.0f32;
    let mut mem = 0u64;
    let mut disk = 0.0f64;
    let mut total_disk = 0u64;
    let mut high_impact = 0usize;
    let mut groups: HashMap<String, BackgroundGroup> = HashMap::new();

    for p in processes {
        total_disk = total_disk.saturating_add(p.disk_bytes_per_sec);
        if p.activity_state != ProcessActivityState::Background {
            continue;
        }
        let w = optional_weight(p);
        if w <= 0.0 {
            continue;
        }
        cpu += p.sustained_cpu_percent * w;
        mem = mem.saturating_add((p.memory_bytes as f64 * w as f64) as u64);
        disk += p.disk_bytes_per_sec as f64 * w as f64;

        let is_high = p.impact_level == LoadImpactLevel::High || p.status == ProcessStatus::Attention;
        if is_high {
            high_impact += 1;
        }

        let key = p.name.to_lowercase();
        let group = groups.entry(key).or_insert_with(|| BackgroundGroup {
            name: display_name(p),
            instances: 0,
            primary_pid: p.pid,
            cpu: 0.0,
            memory_bytes: 0,
            weighted_cpu: 0.0,
            weighted_memory_bytes: 0,
            high_impact: false,
        });
        group.instances += 1;
        group.cpu += p.sustained_cpu_percent;
        group.memory_bytes = group.memory_bytes.saturating_add(p.memory_bytes);
        group.weighted_cpu += p.sustained_cpu_percent * w;
        group.weighted_memory_bytes =
            group.weighted_memory_bytes.saturating_add((p.memory_bytes as f64 * w as f64) as u64);
        group.high_impact |= is_high;
    }

    let mut groups: Vec<BackgroundGroup> = groups.into_values().collect();
    // Rank by how much of the machine the program takes: CPU points plus memory in GB.
    let rank = |g: &BackgroundGroup| g.weighted_cpu + g.weighted_memory_bytes as f32 / (1024.0 * 1024.0 * 1024.0) * 4.0;
    groups.sort_by(|a, b| rank(b).total_cmp(&rank(a)));

    BackgroundAnalysis {
        avoidable_cpu_percent: cpu,
        avoidable_memory_bytes: mem,
        avoidable_disk_bytes_per_sec: disk as u64,
        avoidable_disk_share: if total_disk > 0 { (disk / total_disk as f64).clamp(0.0, 1.0) as f32 } else { 0.0 },
        high_impact_process_count: high_impact,
        groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::ResourceLevel;

    pub fn process(name: &str, category: ProcessCategory, state: ProcessActivityState, cpu: f32, mem_mb: u64) -> ProcessInfo {
        ProcessInfo {
            pid: 100,
            name: name.into(),
            executable_path: None,
            parent_pid: None,
            publisher: None,
            product_name: None,
            cpu_percent: cpu,
            sustained_cpu_percent: cpu,
            memory_bytes: mem_mb * 1024 * 1024,
            disk_bytes_per_sec: 0,
            thread_count: 1,
            category,
            status: ProcessStatus::Normal,
            activity_state: state,
            sustained_load_seconds: 10,
            background_impact_score: 0.0,
            impact_level: LoadImpactLevel::Low,
            cpu_level: ResourceLevel::Normal,
            memory_level: ResourceLevel::Normal,
            is_startup: false,
            is_system_critical: false,
            is_restricted: false,
            started_seconds_ago: None,
            description: None,
        }
    }

    #[test]
    fn windows_components_and_foreground_work_are_not_avoidable() {
        let list = vec![
            process("svchost.exe", ProcessCategory::WindowsCore, ProcessActivityState::Background, 20.0, 500),
            process("code.exe", ProcessCategory::Development, ProcessActivityState::Foreground, 30.0, 900),
        ];
        let a = analyze(&list);
        assert_eq!(a.avoidable_cpu_percent, 0.0);
        assert_eq!(a.avoidable_memory_bytes, 0);
        assert!(a.groups.is_empty());
    }

    #[test]
    fn optional_background_programs_are_weighted_by_importance() {
        let list = vec![
            process("updater.exe", ProcessCategory::Unknown, ProcessActivityState::Background, 10.0, 400),
            process("slack.exe", ProcessCategory::Communication, ProcessActivityState::Background, 10.0, 400),
        ];
        let a = analyze(&list);
        assert!((a.avoidable_cpu_percent - (10.0 + 7.0)).abs() < 1e-3);
        assert_eq!(a.groups.len(), 2);
        assert_eq!(a.groups[0].name, "Updater");
    }

    #[test]
    fn instances_of_one_program_are_grouped() {
        let list = vec![
            process("chrome.exe", ProcessCategory::Browser, ProcessActivityState::Background, 1.0, 300),
            process("chrome.exe", ProcessCategory::Browser, ProcessActivityState::Background, 2.0, 200),
        ];
        let a = analyze(&list);
        assert_eq!(a.groups.len(), 1);
        assert_eq!(a.groups[0].instances, 2);
        assert_eq!(a.groups[0].memory_bytes, 500 * 1024 * 1024);
    }
}
