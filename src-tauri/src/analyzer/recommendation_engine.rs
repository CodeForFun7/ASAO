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

        let mut recommendations: Vec<WidgetRecommendation> = Vec::new();
        let mut seen_pids: std::collections::HashSet<u32> = std::collections::HashSet::new();

        // Collect background process recommendations
        for proc in background_candidates.iter().take(4) {
            let mem_mb = proc.memory_bytes as f64 / (1024.0 * 1024.0);
            if proc.sustained_load_seconds >= 3
                && (proc.sustained_cpu_percent >= 10.0 || proc.background_impact_score >= 35.0)
            {
                seen_pids.insert(proc.pid);
                recommendations.push(WidgetRecommendation {
                    id: format!("bg-sustained-{}", proc.pid),
                    title: "Sustained Background Load".to_string(),
                    message: format!(
                        "{} is consuming {:.1}% CPU and {:.0} MB RAM in the background while inactive.",
                        proc.name, proc.sustained_cpu_percent, mem_mb
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
                });
            } else if proc.memory_bytes >= 200 * 1024 * 1024 {
                seen_pids.insert(proc.pid);
                let formatted_mem = if mem_mb >= 1024.0 {
                    format!("{:.1} GB", mem_mb / 1024.0)
                } else {
                    format!("{:.0} MB", mem_mb)
                };
                recommendations.push(WidgetRecommendation {
                    id: format!("bg-mem-{}", proc.pid),
                    title: "Background Memory Footprint".to_string(),
                    message: format!(
                        "{} is holding {} RAM in the background while not in active focus.",
                        proc.name, formatted_mem
                    ),
                    priority: if proc.memory_bytes >= 900 * 1024 * 1024 {
                        "important".to_string()
                    } else {
                        "interesting".to_string()
                    },
                    process_pid: Some(proc.pid),
                    process_name: Some(proc.name.clone()),
                    metric_highlight: Some(formatted_mem),
                    impact_level: Some(proc.impact_level),
                    sustained_seconds: Some(proc.sustained_load_seconds),
                });
            }
        }

        // Add top memory consumer insight if not already included
        if let Some(top_mem) = by_mem.first().copied() {
            if !seen_pids.contains(&top_mem.pid) && top_mem.memory_bytes >= 150 * 1024 * 1024 {
                seen_pids.insert(top_mem.pid);
                let mb = top_mem.memory_bytes as f64 / (1024.0 * 1024.0);
                let formatted = if mb >= 1024.0 {
                    format!("{:.1} GB", mb / 1024.0)
                } else {
                    format!("{:.0} MB", mb)
                };
                recommendations.push(WidgetRecommendation {
                    id: format!("top-mem-{}", top_mem.pid),
                    title: "Largest Memory Consumer".to_string(),
                    message: format!(
                        "{} is currently the largest memory consumer at {} RAM.",
                        top_mem.name, formatted
                    ),
                    priority: if mb >= 1024.0 {
                        "interesting".to_string()
                    } else {
                        "normal".to_string()
                    },
                    process_pid: Some(top_mem.pid),
                    process_name: Some(top_mem.name.clone()),
                    metric_highlight: Some(formatted),
                    impact_level: Some(top_mem.impact_level),
                    sustained_seconds: Some(top_mem.sustained_load_seconds),
                });
            }
        }

        // Add top CPU consumer insight if not already included
        if let Some(top_cpu) = by_cpu.first().copied() {
            if !seen_pids.contains(&top_cpu.pid) && top_cpu.sustained_cpu_percent >= 5.0 {
                seen_pids.insert(top_cpu.pid);
                recommendations.push(WidgetRecommendation {
                    id: format!("top-cpu-{}", top_cpu.pid),
                    title: "Active CPU Workload".to_string(),
                    message: format!(
                        "{} is leading CPU activity at {:.1}% sustained utilization.",
                        top_cpu.name, top_cpu.sustained_cpu_percent
                    ),
                    priority: if top_cpu.sustained_cpu_percent >= 35.0 {
                        "interesting".to_string()
                    } else {
                        "normal".to_string()
                    },
                    process_pid: Some(top_cpu.pid),
                    process_name: Some(top_cpu.name.clone()),
                    metric_highlight: Some(format!("{:.1}% CPU", top_cpu.sustained_cpu_percent)),
                    impact_level: Some(top_cpu.impact_level),
                    sustained_seconds: Some(top_cpu.sustained_load_seconds),
                });
            }
        }

        // Add system-level memory/strain recommendation
        if m.memory_usage_percent >= 70.0 {
            recommendations.push(WidgetRecommendation {
                id: "sys-ram-pressure".to_string(),
                title: "Elevated Memory Utilization".to_string(),
                message: format!(
                    "System RAM is at {:.0}% capacity across {} active processes. Closing unused apps can free memory.",
                    m.memory_usage_percent, m.total_processes
                ),
                priority: if m.memory_usage_percent >= 85.0 {
                    "important".to_string()
                } else {
                    "interesting".to_string()
                },
                process_pid: None,
                process_name: None,
                metric_highlight: Some(format!("{:.0}% RAM", m.memory_usage_percent)),
                impact_level: Some(LoadImpactLevel::Moderate),
                sustained_seconds: Some(0),
            });
        } else if recommendations.len() < 2 {
            recommendations.push(WidgetRecommendation {
                id: "system-nominal".to_string(),
                title: "Background Load Nominal".to_string(),
                message: format!(
                    "Background load is steady at {:.0}% across {} running processes.",
                    m.background_load_percent, m.total_processes
                ),
                priority: "normal".to_string(),
                process_pid: None,
                process_name: None,
                metric_highlight: Some(format!("{:.0}% BG", m.background_load_percent)),
                impact_level: Some(LoadImpactLevel::Low),
                sustained_seconds: Some(0),
            });
        }

        let recommendation = recommendations.first().cloned().unwrap_or(WidgetRecommendation {
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
        });

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
            recommendations,
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
