use crate::analyzer::{
    CompactProcessContext, LoadImpactLevel, ProcessActivityState, ProcessInfo,
    ProcessSnapshotPayload, WidgetRecommendation, WidgetSystemUpdate,
};

pub struct RecommendationEngine;

impl RecommendationEngine {
    /// Evaluates the analyzed system snapshot using deterministic local rules to identify
    /// processes that unnecessarily make the PC heavier in the background while the user
    /// is not actively using them. Never modifies or terminates any process.
    pub fn evaluate_snapshot(snapshot: &ProcessSnapshotPayload) -> WidgetSystemUpdate {
        let m = &snapshot.metrics;

        // 1. Evaluate overall system condition factoring in background load and strain
        let (condition, condition_reason) = if m.system_strain_percent >= 78.0
            || m.background_load_percent >= 35.0
            || m.attention_processes >= 5
        {
            (
                "ATTENTION".to_string(),
                format!(
                    "Background processes are adding {:.0}% load ({:.0}% strain, {} flagged)",
                    m.background_load_percent, m.system_strain_percent, m.attention_processes
                ),
            )
        } else if m.system_strain_percent >= 52.0
            || m.background_load_percent >= 18.0
            || m.attention_processes >= 2
        {
            (
                "ELEVATED".to_string(),
                format!(
                    "Moderate background load ({:.0}% background, {:.0}% total CPU)",
                    m.background_load_percent, m.cpu_usage_percent
                ),
            )
        } else {
            (
                "GOOD".to_string(),
                format!(
                    "Background load is low ({:.0}% background · {:.0}% strain).",
                    m.background_load_percent, m.system_strain_percent
                ),
            )
        };

        // 2. Build compact context lists for top CPU and top Memory processes
        let mut by_cpu: Vec<&ProcessInfo> = snapshot.processes.iter().collect();
        by_cpu.sort_by(|a, b| {
            b.sustained_cpu_percent
                .partial_cmp(&a.sustained_cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut by_mem: Vec<&ProcessInfo> = snapshot.processes.iter().collect();
        by_mem.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));

        let top_cpu_processes: Vec<CompactProcessContext> = by_cpu
            .iter()
            .take(5)
            .map(|p| Self::to_compact_context(p))
            .collect();

        let top_memory_processes: Vec<CompactProcessContext> = by_mem
            .iter()
            .take(5)
            .map(|p| Self::to_compact_context(p))
            .collect();

        // 3. Deterministic Recommendation Rules:
        // Prioritize non-critical BACKGROUND processes with sustained resource consumption
        // while the user is not actively using them.
        let mut background_candidates: Vec<&ProcessInfo> = snapshot
            .processes
            .iter()
            .filter(|p| {
                !p.is_system_critical && p.activity_state == ProcessActivityState::Background
            })
            .collect();

        background_candidates.sort_by(|a, b| {
            b.background_impact_score
                .partial_cmp(&a.background_impact_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let top_bg_candidate = background_candidates.first().copied();

        let recommendation = if let Some(proc) = top_bg_candidate.filter(|p| {
            p.sustained_load_seconds >= 3
                && (p.sustained_cpu_percent >= 14.0 || p.background_impact_score >= 42.0)
        }) {
            let mem_mb = proc.memory_bytes as f64 / (1024.0 * 1024.0);
            WidgetRecommendation {
                id: format!("bg-sustained-{}", proc.pid),
                title: "Sustained Background Load".to_string(),
                message: format!(
                    "{} is consuming {:.1}% CPU and {:.0} MB RAM in the background while not in active focus ({}s sustained).",
                    proc.name,
                    proc.sustained_cpu_percent,
                    mem_mb,
                    proc.sustained_load_seconds.max(3)
                ),
                priority: if proc.impact_level == LoadImpactLevel::High {
                    "important".to_string()
                } else {
                    "interesting".to_string()
                },
                process_pid: Some(proc.pid),
                process_name: Some(proc.name.clone()),
                metric_highlight: Some(format!("{:.1}% BG CPU", proc.sustained_cpu_percent)),
                impact_level: Some(proc.impact_level),
                sustained_seconds: Some(proc.sustained_load_seconds),
            }
        } else if let Some(proc) =
            top_bg_candidate.filter(|p| p.memory_bytes >= 400 * 1024 * 1024)
        {
            let mb = proc.memory_bytes as f64 / (1024.0 * 1024.0);
            let formatted_mem = if mb >= 1024.0 {
                format!("{:.1} GB", mb / 1024.0)
            } else {
                format!("{:.0} MB", mb)
            };
            WidgetRecommendation {
                id: format!("bg-mem-{}", proc.pid),
                title: "High Background Memory Footprint".to_string(),
                message: format!(
                    "{} is holding {} RAM in the background while inactive.",
                    proc.name, formatted_mem
                ),
                priority: if proc.memory_bytes >= 1100 * 1024 * 1024 {
                    "important".to_string()
                } else {
                    "interesting".to_string()
                },
                process_pid: Some(proc.pid),
                process_name: Some(proc.name.clone()),
                metric_highlight: Some(formatted_mem),
                impact_level: Some(proc.impact_level),
                sustained_seconds: Some(proc.sustained_load_seconds),
            }
        } else if let Some(proc) = top_bg_candidate {
            let mb = (proc.memory_bytes as f64 / (1024.0 * 1024.0)).max(1.0);
            WidgetRecommendation {
                id: format!("bg-nominal-{}", proc.pid),
                title: "Background Load Nominal".to_string(),
                message: format!(
                    "{} is the largest background app at {:.0} MB RAM ({:.1}% CPU).",
                    proc.name, mb, proc.sustained_cpu_percent
                ),
                priority: "normal".to_string(),
                process_pid: Some(proc.pid),
                process_name: Some(proc.name.clone()),
                metric_highlight: Some(format!("{:.0} MB", mb)),
                impact_level: Some(LoadImpactLevel::Low),
                sustained_seconds: Some(proc.sustained_load_seconds),
            }
        } else {
            WidgetRecommendation {
                id: "system-nominal".to_string(),
                title: "No Unnecessary Background Load".to_string(),
                message: "Background processes are quiet and operating within nominal resource limits."
                    .to_string(),
                priority: "normal".to_string(),
                process_pid: None,
                process_name: None,
                metric_highlight: None,
                impact_level: Some(LoadImpactLevel::Low),
                sustained_seconds: Some(0),
            }
        };

        WidgetSystemUpdate {
            cpu_usage: m.cpu_usage_percent,
            memory_usage: m.memory_usage_percent,
            memory_used_bytes: m.memory_used_bytes,
            memory_total_bytes: m.memory_total_bytes,
            gpu_usage: m.gpu_usage_percent,
            foreground_load: m.foreground_load_percent,
            background_load: m.background_load_percent,
            system_strain: m.system_strain_percent,
            user_active: m.user_active,
            foreground_process_name: m.foreground_process_name.clone(),
            process_count: m.total_processes,
            attention_count: m.attention_processes,
            condition,
            condition_reason,
            recommendation,
            top_cpu_processes,
            top_memory_processes,
            timestamp: m.timestamp_ms,
        }
    }

    fn to_compact_context(p: &ProcessInfo) -> CompactProcessContext {
        CompactProcessContext {
            pid: p.pid,
            name: p.name.clone(),
            category: p.category,
            cpu_percent: p.cpu_percent,
            sustained_cpu_percent: p.sustained_cpu_percent,
            memory_bytes: p.memory_bytes,
            status: p.status,
            activity_state: p.activity_state,
            sustained_load_seconds: p.sustained_load_seconds,
            impact_level: p.impact_level,
        }
    }
}
