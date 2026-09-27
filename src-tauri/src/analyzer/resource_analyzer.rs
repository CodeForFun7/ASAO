use crate::analyzer::{
    LoadImpactLevel, ProcessActivityState, ProcessCategory, ProcessStatus, ResourceLevel,
};

/// Centralized resource and background load thresholds
pub mod thresholds {
    pub const CPU_NORMAL_MAX: f32 = 10.0;
    pub const CPU_MODERATE_MAX: f32 = 25.0;
    pub const CPU_HIGH_MAX: f32 = 50.0;

    pub const MEM_NORMAL_MAX_BYTES: u64 = 250 * 1024 * 1024; // 250 MB
    pub const MEM_MODERATE_MAX_BYTES: u64 = 600 * 1024 * 1024; // 600 MB
    pub const MEM_HIGH_MAX_BYTES: u64 = 1200 * 1024 * 1024; // 1.2 GB

    pub const ATTENTION_SUSTAINED_CPU: f32 = 16.0;
    pub const ATTENTION_MEMORY_BYTES: u64 = 750 * 1024 * 1024; // 750 MB in background
    pub const HIGH_RESOURCE_CPU: f32 = 10.0;
    pub const HIGH_RESOURCE_MEMORY_BYTES: u64 = 450 * 1024 * 1024; // 450 MB
}

pub fn classify_cpu_level(sustained_cpu_percent: f32) -> ResourceLevel {
    if sustained_cpu_percent < thresholds::CPU_NORMAL_MAX {
        ResourceLevel::Normal
    } else if sustained_cpu_percent < thresholds::CPU_MODERATE_MAX {
        ResourceLevel::Moderate
    } else if sustained_cpu_percent < thresholds::CPU_HIGH_MAX {
        ResourceLevel::High
    } else {
        ResourceLevel::VeryHigh
    }
}

pub fn classify_memory_level(memory_bytes: u64) -> ResourceLevel {
    if memory_bytes < thresholds::MEM_NORMAL_MAX_BYTES {
        ResourceLevel::Normal
    } else if memory_bytes < thresholds::MEM_MODERATE_MAX_BYTES {
        ResourceLevel::Moderate
    } else if memory_bytes < thresholds::MEM_HIGH_MAX_BYTES {
        ResourceLevel::High
    } else {
        ResourceLevel::VeryHigh
    }
}

/// Determines whether a process is actively in the foreground, consuming resources in the
/// background without user focus, or dormant/inactive.
pub fn determine_activity_state(
    is_foreground_family: bool,
    seconds_since_foreground: Option<u64>,
    current_cpu: f32,
    sustained_cpu: f32,
    memory_bytes: u64,
    disk_bytes_per_sec: u64,
    network_bytes_per_sec: u64,
) -> ProcessActivityState {
    if is_foreground_family || seconds_since_foreground.is_some_and(|secs| secs <= 15) {
        return ProcessActivityState::Foreground;
    }

    if sustained_cpu >= 0.4
        || current_cpu >= 0.8
        || memory_bytes >= 180 * 1024 * 1024
        || disk_bytes_per_sec >= 128 * 1024
        || network_bytes_per_sec >= 64 * 1024
    {
        ProcessActivityState::Background
    } else {
        ProcessActivityState::Inactive
    }
}

/// Computes a 0..100 score quantifying how much a process unnecessarily makes the PC heavier
/// in the background while the user is not actively using it.
pub fn compute_background_impact_score(
    activity_state: ProcessActivityState,
    is_system_critical: bool,
    sustained_cpu: f32,
    memory_bytes: u64,
    disk_bytes_per_sec: u64,
    sustained_load_seconds: u64,
) -> f32 {
    if is_system_critical {
        return 0.0;
    }

    let mem_mb = memory_bytes as f32 / (1024.0 * 1024.0);
    let disk_mb_s = disk_bytes_per_sec as f32 / (1024.0 * 1024.0);

    // Base resource pressure from sustained CPU, RAM footprint, and Disk I/O
    let resource_score =
        (sustained_cpu * 1.8) + ((mem_mb / 1200.0) * 42.0) + (disk_mb_s * 3.5);

    // Duration multiplier rewards sustained trends rather than brief 1-second spikes
    let duration_factor = match sustained_load_seconds {
        0..=2 => 0.45,
        3..=6 => 0.85,
        7..=15 => 1.10,
        _ => 1.25,
    };

    // Foreground processes are actively used by the user, so their background impact is dampened
    let activity_factor = match activity_state {
        ProcessActivityState::Foreground => 0.20,
        ProcessActivityState::Background => 1.0,
        ProcessActivityState::Inactive => 0.15,
    };

    (resource_score * duration_factor * activity_factor).clamp(0.0, 100.0)
}

pub fn classify_impact_level(background_impact_score: f32) -> LoadImpactLevel {
    if background_impact_score >= 45.0 {
        LoadImpactLevel::High
    } else if background_impact_score >= 20.0 {
        LoadImpactLevel::Moderate
    } else {
        LoadImpactLevel::Low
    }
}

/// Evaluates process status using sustained CPU average, memory footprint, foreground/background
/// activity state, and duration so single spikes or active foreground work are not falsely
/// flagged as unnecessary background load.
pub fn determine_process_status(
    category: ProcessCategory,
    is_system_critical: bool,
    activity_state: ProcessActivityState,
    current_cpu: f32,
    sustained_cpu: f32,
    sustained_load_seconds: u64,
    memory_bytes: u64,
    disk_bytes_per_sec: u64,
    background_impact_score: f32,
) -> ProcessStatus {
    if is_system_critical {
        return ProcessStatus::Protected;
    }

    // Core Goal: Flag processes that unnecessarily make the PC heavier in the background
    // while the user is not actively using them (sustained over time, not single spikes).
    if activity_state == ProcessActivityState::Background
        && (background_impact_score >= 45.0
            || (sustained_load_seconds >= 3 && sustained_cpu >= thresholds::ATTENTION_SUSTAINED_CPU)
            || memory_bytes >= thresholds::ATTENTION_MEMORY_BYTES
            || (category == ProcessCategory::Unknown
                && sustained_load_seconds >= 3
                && (sustained_cpu >= 8.0 || memory_bytes >= 350 * 1024 * 1024)))
    {
        return ProcessStatus::Attention;
    }

    if sustained_cpu >= thresholds::HIGH_RESOURCE_CPU
        || memory_bytes >= thresholds::HIGH_RESOURCE_MEMORY_BYTES
    {
        return ProcessStatus::HighResource;
    }

    if activity_state == ProcessActivityState::Foreground
        || current_cpu >= 1.2
        || disk_bytes_per_sec > 256 * 1024
    {
        return ProcessStatus::Active;
    }

    if activity_state == ProcessActivityState::Inactive
        || (current_cpu < 0.2 && memory_bytes < 120 * 1024 * 1024)
    {
        return ProcessStatus::Background;
    }

    ProcessStatus::Normal
}

