pub mod classifier;
pub mod process_analyzer;
pub mod recommendation_engine;
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
pub enum ProcessActivityState {
    Foreground,
    Background,
    Inactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoadImpactLevel {
    Low,
    Moderate,
    High,
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
    pub sustained_cpu_percent: f32,
    pub gpu_percent: f32,
    pub memory_bytes: u64,
    pub disk_bytes_per_sec: u64,
    pub network_bytes_per_sec: u64,
    pub thread_count: u32,
    pub category: ProcessCategory,
    pub status: ProcessStatus,
    pub activity_state: ProcessActivityState,
    pub sustained_load_seconds: u64,
    pub background_impact_score: f32,
    pub impact_level: LoadImpactLevel,
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
    pub disk_bytes_per_sec: u64,
    pub network_bytes_per_sec: u64,
    pub foreground_load_percent: f32,
    pub background_load_percent: f32,
    pub system_strain_percent: f32,
    pub user_active: bool,
    pub foreground_process_name: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactProcessContext {
    pub pid: u32,
    pub name: String,
    pub category: ProcessCategory,
    pub cpu_percent: f32,
    pub sustained_cpu_percent: f32,
    pub memory_bytes: u64,
    pub status: ProcessStatus,
    pub activity_state: ProcessActivityState,
    pub sustained_load_seconds: u64,
    pub impact_level: LoadImpactLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetRecommendation {
    pub id: String,
    pub title: String,
    pub message: String,
    pub priority: String, // "normal" | "interesting" | "important"
    pub process_pid: Option<u32>,
    pub process_name: Option<String>,
    pub metric_highlight: Option<String>,
    pub impact_level: Option<LoadImpactLevel>,
    pub sustained_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetSystemUpdate {
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub gpu_usage: f32,
    pub foreground_load: f32,
    pub background_load: f32,
    pub system_strain: f32,
    pub user_active: bool,
    pub foreground_process_name: Option<String>,
    pub process_count: usize,
    pub attention_count: usize,
    pub condition: String, // "GOOD" | "ELEVATED" | "ATTENTION"
    pub condition_reason: String,
    pub recommendation: WidgetRecommendation,
    pub top_cpu_processes: Vec<CompactProcessContext>,
    pub top_memory_processes: Vec<CompactProcessContext>,
    pub timestamp: u64,
}

