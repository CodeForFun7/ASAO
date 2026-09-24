use crate::analyzer::{ProcessCategory, ProcessStatus, ResourceLevel};

/// Centralized resource analysis thresholds
pub mod thresholds {
    pub const CPU_NORMAL_MAX: f32 = 10.0;
    pub const CPU_MODERATE_MAX: f32 = 25.0;
    pub const CPU_HIGH_MAX: f32 = 50.0;

    pub const MEM_NORMAL_MAX_BYTES: u64 = 250 * 1024 * 1024; // 250 MB
    pub const MEM_MODERATE_MAX_BYTES: u64 = 600 * 1024 * 1024; // 600 MB
    pub const MEM_HIGH_MAX_BYTES: u64 = 1200 * 1024 * 1024; // 1.2 GB

    pub const ATTENTION_SUSTAINED_CPU: f32 = 22.0;
    pub const ATTENTION_MEMORY_BYTES: u64 = 900 * 1024 * 1024; // 900 MB
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

/// Evaluates process status using recent sustained CPU average and current memory/IO metrics
/// so a single spike does not falsely classify a process as problematic.
pub fn determine_process_status(
    category: ProcessCategory,
    is_system_critical: bool,
    current_cpu: f32,
    sustained_cpu: f32,
    memory_bytes: u64,
    disk_bytes_per_sec: u64,
) -> ProcessStatus {
    if is_system_critical {
        return ProcessStatus::Protected;
    }

    // Flag for attention if sustained CPU is high, memory footprint is very large,
    // or an unclassified process has notable sustained activity
    if sustained_cpu >= thresholds::ATTENTION_SUSTAINED_CPU
        || memory_bytes >= thresholds::ATTENTION_MEMORY_BYTES
        || (category == ProcessCategory::Unknown
            && (sustained_cpu >= 8.0 || memory_bytes >= 350 * 1024 * 1024))
    {
        return ProcessStatus::Attention;
    }

    if sustained_cpu >= thresholds::HIGH_RESOURCE_CPU
        || memory_bytes >= thresholds::HIGH_RESOURCE_MEMORY_BYTES
    {
        return ProcessStatus::HighResource;
    }

    if current_cpu >= 1.2 || disk_bytes_per_sec > 256 * 1024 {
        return ProcessStatus::Active;
    }

    if current_cpu < 0.2 && memory_bytes < 120 * 1024 * 1024 {
        return ProcessStatus::Background;
    }

    ProcessStatus::Normal
}
