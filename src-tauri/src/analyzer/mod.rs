pub mod classifier;
pub mod process_analyzer;
pub mod resource_analyzer;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessCategory {
    WindowsCore,
    Drivers,
    Gaming,
    Development,
    Productivity,
    Communication,
    Browser,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessStatus {
    Normal,
    Active,
    HighResource,
    Background,
    Attention,
    Protected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceLevel {
    Normal,
    Moderate,
    High,
    VeryHigh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub executable_path: Option<String>,
    pub parent_pid: Option<u32>,
    pub publisher: Option<String>,
    pub product_name: Option<String>,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub disk_bytes_per_sec: u64,
    pub network_bytes_per_sec: u64,
    pub thread_count: u32,
    pub category: ProcessCategory,
    pub status: ProcessStatus,
    pub cpu_level: ResourceLevel,
    pub memory_level: ResourceLevel,
    pub is_startup: bool,
    pub is_system_critical: bool,
    pub is_restricted: bool,
    pub started_seconds_ago: Option<u64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub cpu_delta_percent: f32,
    pub memory_usage_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_delta_percent: f32,
    pub gpu_usage_percent: f32,
    pub gpu_delta_percent: f32,
    pub total_processes: usize,
    pub attention_processes: usize,
    pub protected_processes: usize,
    pub high_resource_processes: usize,
    pub system_status: String, // "healthy" | "warning" | "critical"
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshotPayload {
    pub processes: Vec<ProcessInfo>,
    pub metrics: SystemMetrics,
}
