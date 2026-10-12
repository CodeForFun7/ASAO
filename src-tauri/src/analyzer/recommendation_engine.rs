use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::analyzer::storage_analyzer::{
    compute_quick_dir_size, StorageAnalyzerEngine, StorageCategory, StorageItem,
};
use crate::analyzer::{
    CompactProcessContext, LoadImpactLevel, ProcessActivityState, ProcessCategory, ProcessInfo,
    ProcessSnapshotPayload, WidgetRecommendation, WidgetSystemUpdate,
};
use crate::collector::storage::{normalize_win_path, system_time_to_ms};
use crate::startup::scanner::{ItemClass, Recommendation, StartupImpact, StartupItem};

pub struct RecommendationEngine;

impl RecommendationEngine {
    /// Evaluates the analyzed system snapshot using deterministic local rules across Processes,
    /// and merges any cached Startup and Storage recommendations for the Widget carousel.
    pub fn evaluate_snapshot(snapshot: &ProcessSnapshotPayload) -> WidgetSystemUpdate {
        Self::evaluate_snapshot_with_extras(snapshot, &[], &[])
    }

    pub fn evaluate_snapshot_with_extras(
        snapshot: &ProcessSnapshotPayload,
        startup_recs: &[WidgetRecommendation],
        storage_recs: &[WidgetRecommendation],
    ) -> WidgetSystemUpdate {
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

        // 3. Evaluate Process Recommendations
        let proc_recs = Self::evaluate_process_recommendations(snapshot);

        // 4. Interleave Process, Startup, and Storage recommendations so the Widget
        // interval scroll showcases insights across all three domains.
        let mut combined: Vec<WidgetRecommendation> = Vec::new();
        let max_len = proc_recs
            .len()
            .max(startup_recs.len())
            .max(storage_recs.len());

        for i in 0..max_len {
            if let Some(pr) = proc_recs.get(i) {
                combined.push(pr.clone());
            }
            if let Some(sr) = startup_recs.get(i) {
                combined.push(sr.clone());
            }
            if let Some(str_rec) = storage_recs.get(i) {
                combined.push(str_rec.clone());
            }
        }

        if combined.is_empty() {
            combined.push(WidgetRecommendation {
                id: "system-nominal".to_string(),
                category: "process".to_string(),
                sub_category: Some("System Nominal".to_string()),
                title: "Background Load Nominal".to_string(),
                message: format!(
                    "Background load is steady at {:.0}% across {} running processes.",
                    m.background_load_percent, m.total_processes
                ),
                priority: "normal".to_string(),
                action_label: None,
                process_pid: None,
                process_name: None,
                startup_item_id: None,
                storage_path: None,
                metric_highlight: Some(format!("{:.0}% BG", m.background_load_percent)),
                impact_level: Some(LoadImpactLevel::Low),
                sustained_seconds: Some(0),
            });
        }

        let recommendation = combined.first().cloned().unwrap();

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
            recommendations: combined,
            top_cpu_processes,
            top_memory_processes,
            timestamp: m.timestamp_ms,
        }
    }

    /// Native Rust inference for Process Recommendations:
    /// Identifies high-resource processes that are idle in the background with no recent user activity
    /// (e.g., 30+ minutes inactive) and recommends closing them to preserve system resources.
    pub fn evaluate_process_recommendations(
        snapshot: &ProcessSnapshotPayload,
    ) -> Vec<WidgetRecommendation> {
        let mut recommendations: Vec<WidgetRecommendation> = Vec::new();
        let mut seen_pids: HashSet<u32> = HashSet::new();

        // Filter non-core, non-critical background/inactive processes
        let mut idle_candidates: Vec<&ProcessInfo> = snapshot
            .processes
            .iter()
            .filter(|p| {
                !p.is_system_critical
                    && p.category != ProcessCategory::WindowsCore
                    && p.category != ProcessCategory::Drivers
                    && !p.name.eq_ignore_ascii_case("asao.exe")
                    && !p.name.eq_ignore_ascii_case("explorer.exe")
                    && (p.activity_state == ProcessActivityState::Background
                        || p.activity_state == ProcessActivityState::Inactive)
                    && (p.memory_bytes >= 80 * 1024 * 1024
                        || p.sustained_cpu_percent >= 2.5
                        || p.background_impact_score >= 18.0)
            })
            .collect();

        // Sort by combined resource impact (memory + CPU + background score)
        idle_candidates.sort_by(|a, b| {
            let score_a = (a.memory_bytes as f64 / (1024.0 * 1024.0))
                + (a.sustained_cpu_percent as f64 * 25.0)
                + (a.background_impact_score as f64 * 5.0);
            let score_b = (b.memory_bytes as f64 / (1024.0 * 1024.0))
                + (b.sustained_cpu_percent as f64 * 25.0)
                + (b.background_impact_score as f64 * 5.0);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for proc in idle_candidates.iter().take(6) {
            seen_pids.insert(proc.pid);
            let mem_str = Self::format_bytes(proc.memory_bytes);
            let idle_minutes = proc
                .started_seconds_ago
                .map(|secs| (secs / 60).max(30))
                .unwrap_or(30);

            let (sub_cat, title, msg) = if proc.sustained_cpu_percent >= 5.0 {
                (
                    "Idle High-CPU & RAM Process (30m+ Inactive)",
                    format!("Close Idle Resource Hog: {}", proc.name),
                    format!(
                        "This process ({}) is consuming a lot of resources ({:.1}% CPU and {} RAM), is idle, and there has been no user activity in the past {} minutes. I recommend you to close this to preserve resources.",
                        proc.name, proc.sustained_cpu_percent, mem_str, idle_minutes
                    ),
                )
            } else {
                (
                    "Idle Memory Consumer (30m+ Inactive)",
                    format!("Idle Resource Consumer: {}", proc.name),
                    format!(
                        "This process ({}) is consuming a lot of resources ({} RAM, {:.1}% CPU), is idle, and there has been no user activity in the past {} minutes. I recommend you to close this to preserve resources.",
                        proc.name, mem_str, proc.sustained_cpu_percent, idle_minutes
                    ),
                )
            };

            let highlight = if proc.sustained_cpu_percent >= 2.0 {
                format!("{} · {:.1}% CPU", mem_str, proc.sustained_cpu_percent)
            } else {
                format!("{} RAM · Idle {}m", mem_str, idle_minutes)
            };

            recommendations.push(WidgetRecommendation {
                id: format!("proc-idle-{}", proc.pid),
                category: "process".to_string(),
                sub_category: Some(sub_cat.to_string()),
                title,
                message: msg,
                priority: if proc.memory_bytes >= 500 * 1024 * 1024
                    || proc.sustained_cpu_percent >= 12.0
                {
                    "important".to_string()
                } else {
                    "interesting".to_string()
                },
                action_label: Some("Close Process to Preserve Resources".to_string()),
                process_pid: Some(proc.pid),
                process_name: Some(proc.name.clone()),
                startup_item_id: None,
                storage_path: None,
                metric_highlight: Some(highlight),
                impact_level: Some(proc.impact_level),
                sustained_seconds: Some(idle_minutes * 60),
            });
        }

        // If fewer than 2 background candidates were found, check overall top non-core memory/CPU consumers
        if recommendations.len() < 2 {
            let mut fallback: Vec<&ProcessInfo> = snapshot
                .processes
                .iter()
                .filter(|p| {
                    !seen_pids.contains(&p.pid)
                        && !p.is_system_critical
                        && p.category != ProcessCategory::WindowsCore
                        && p.memory_bytes >= 60 * 1024 * 1024
                })
                .collect();
            fallback.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes));

            for proc in fallback.into_iter().take(2) {
                let mem_str = Self::format_bytes(proc.memory_bytes);
                recommendations.push(WidgetRecommendation {
                    id: format!("proc-resource-{}", proc.pid),
                    category: "process".to_string(),
                    sub_category: Some("High Resource Consumer".to_string()),
                    title: format!("High Resource Footprint: {}", proc.name),
                    message: format!(
                        "{} is holding {} RAM ({:.1}% CPU). If you have had no active work in this app for the past 30 minutes, consider closing it to free system memory.",
                        proc.name, mem_str, proc.sustained_cpu_percent
                    ),
                    priority: "interesting".to_string(),
                    action_label: Some("Inspect & Close if Unused".to_string()),
                    process_pid: Some(proc.pid),
                    process_name: Some(proc.name.clone()),
                    startup_item_id: None,
                    storage_path: None,
                    metric_highlight: Some(format!("{} RAM", mem_str)),
                    impact_level: Some(proc.impact_level),
                    sustained_seconds: Some(1800),
                });
            }
        }

        recommendations
    }

    /// Native Rust inference for Startup Recommendations:
    /// Identifies enabled startup entries that are heavy on boot/memory and are NOT core Windows
    /// programs or strictly needed at Windows boot.
    pub fn evaluate_startup_recommendations(items: &[StartupItem]) -> Vec<WidgetRecommendation> {
        let mut recommendations: Vec<WidgetRecommendation> = Vec::new();

        let mut candidates: Vec<&StartupItem> = items
            .iter()
            .filter(|item| {
                item.is_enabled
                    && item.classification != ItemClass::Essential
                    && (item.recommendation == Recommendation::Disable
                        || item.recommendation == Recommendation::Investigate
                        || item.impact == StartupImpact::High
                        || item.impact == StartupImpact::Medium
                        || item.boot_duration_ms >= 400
                        || item.memory_bytes >= 50 * 1024 * 1024)
            })
            .collect();

        // Sort by impact tier (High -> Medium -> Low) and then by boot_duration_ms + memory_bytes
        candidates.sort_by(|a, b| {
            let tier = |imp: &StartupImpact| match imp {
                StartupImpact::High => 3,
                StartupImpact::Medium => 2,
                StartupImpact::Low => 1,
            };
            let t_cmp = tier(&b.impact).cmp(&tier(&a.impact));
            if t_cmp != std::cmp::Ordering::Equal {
                return t_cmp;
            }
            let score_a = a.boot_duration_ms + (a.memory_bytes / (1024 * 1024));
            let score_b = b.boot_duration_ms + (b.memory_bytes / (1024 * 1024));
            score_b.cmp(&score_a)
        });

        for item in candidates.into_iter().take(7) {
            let delay_sec = (item.boot_duration_ms as f64 / 1000.0).max(0.2);
            let mem_str = if item.memory_bytes > 0 {
                Self::format_bytes(item.memory_bytes)
            } else {
                "Background Service".to_string()
            };

            let publisher_note = item
                .publisher
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or("Third-Party Publisher");

            let (sub_cat, priority) = if item.impact == StartupImpact::High
                || item.recommendation == Recommendation::Disable
            {
                ("Heavy Non-Core Boot Program", "important")
            } else {
                ("Optional Boot Autostart", "interesting")
            };

            let highlight = if item.memory_bytes > 0 {
                format!("+{:.1}s Boot · {}", delay_sec, mem_str)
            } else {
                format!("+{:.1}s Boot Delay", delay_sec)
            };

            let message = format!(
                "\"{}\" ({}) is a heavy startup entry adding ~{:.1}s to boot time ({}) and is not a core Windows program or required at Windows boot. We recommend disabling it from automatic startup so it only runs when you explicitly launch it.",
                item.name, publisher_note, delay_sec, mem_str
            );

            recommendations.push(WidgetRecommendation {
                id: format!("startup-rec-{}", item.id),
                category: "startup".to_string(),
                sub_category: Some(sub_cat.to_string()),
                title: format!("Heavy Non-Core Startup: {}", item.name),
                message,
                priority: priority.to_string(),
                action_label: Some("Disable from Windows Boot".to_string()),
                process_pid: item.pid,
                process_name: Some(item.name.clone()),
                startup_item_id: Some(item.id.clone()),
                storage_path: item.executable_path.clone(),
                metric_highlight: Some(highlight),
                impact_level: Some(match item.impact {
                    StartupImpact::High => LoadImpactLevel::High,
                    StartupImpact::Medium => LoadImpactLevel::Moderate,
                    StartupImpact::Low => LoadImpactLevel::Low,
                }),
                sustained_seconds: Some(item.boot_duration_ms / 1000),
            });
        }

        recommendations
    }

    /// Native Rust inference for Storage Recommendations:
    /// 1. Identifies user-specific files occupying huge space with no activity for 1+ year (or longest dormant period) -> recommends reviewing.
    /// 2. Identifies application files/folders whose parent application is not installed/present on the PC -> recommends deleting if not needed in future.
    /// 3. Identifies large reclaimable caches & temporary staging directories.
    pub fn evaluate_storage_recommendations(
        storage_engine: &StorageAnalyzerEngine,
        running_procs: &[ProcessInfo],
    ) -> Vec<WidgetRecommendation> {
        let mut recommendations: Vec<WidgetRecommendation> = Vec::new();
        let mut seen_paths: HashSet<String> = HashSet::new();

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let one_year_ms: u64 = 365 * 24 * 60 * 60 * 1000;

        // Gather installed application tokens from C:\Program Files, C:\Program Files (x86), AppData\Local\Programs, and running processes
        let installed_app_tokens = Self::collect_installed_app_tokens(running_procs);

        // Collect candidate storage items from either the active DriveStorageIndex OR a fast native inspection
        let mut candidate_items: Vec<StorageItem> = Vec::new();
        if let Some(ref active_drv) = storage_engine.active_drive {
            if let Some(idx) = storage_engine.indices.get(active_drv) {
                candidate_items.extend(idx.indexed_items.iter().cloned());
                if let Some(ref snap) = idx.snapshot {
                    candidate_items.extend(snap.largest_files.iter().cloned());
                    candidate_items.extend(snap.largest_folders.iter().cloned());
                }
            }
        }
        if candidate_items.is_empty() {
            for idx in storage_engine.indices.values() {
                candidate_items.extend(idx.indexed_items.iter().cloned());
                if let Some(ref snap) = idx.snapshot {
                    candidate_items.extend(snap.largest_files.iter().cloned());
                }
            }
        }

        // Always supplement with fast targeted probing of user folders & AppData so recommendations work even before a manual scan
        let probed_items = Self::fast_probe_user_and_app_storage();
        candidate_items.extend(probed_items);

        // Sort candidate items by size descending
        candidate_items.sort_by(|a, b| b.size.cmp(&a.size));

        // -------------------------------------------------------------------
        // Outcome A: User-specific files occupying large space with no activity since >= 1 year (or longest dormant user files)
        // -------------------------------------------------------------------
        let mut dormant_user_files: Vec<(StorageItem, u64)> = Vec::new();
        for item in &candidate_items {
            if item.is_dir || item.size < 10 * 1024 * 1024 {
                continue;
            }
            let norm_p = item.path.to_lowercase();
            if seen_paths.contains(&norm_p) {
                continue;
            }
            let is_user_specific = item.category == StorageCategory::USER
                || norm_p.contains("\\users\\")
                    && (norm_p.contains("\\downloads\\")
                        || norm_p.contains("\\documents\\")
                        || norm_p.contains("\\videos\\")
                        || norm_p.contains("\\desktop\\")
                        || norm_p.contains("\\pictures\\"));

            if !is_user_specific {
                continue;
            }

            // Use the most recent activity timestamp between modified_ms and created_ms
            let last_activity_ms = item.modified_ms.or(item.accessed_ms).or(item.created_ms);
            if let Some(ts) = last_activity_ms {
                let inactive_ms = now_ms.saturating_sub(ts);
                let inactive_days = inactive_ms / (1000 * 60 * 60 * 24);
                if inactive_ms >= one_year_ms || inactive_days >= 30 {
                    seen_paths.insert(norm_p);
                    dormant_user_files.push((item.clone(), inactive_days));
                }
            }
        }

        // Prioritize files with >= 365 days (1 year) inactivity first, then by size
        dormant_user_files.sort_by(|a, b| {
            let a_over_year = a.1 >= 365;
            let b_over_year = b.1 >= 365;
            b_over_year
                .cmp(&a_over_year)
                .then_with(|| b.0.size.cmp(&a.0.size))
        });

        for (item, inactive_days) in dormant_user_files.into_iter().take(3) {
            let size_str = Self::format_bytes(item.size);
            let (sub_cat, inactivity_phrase, highlight) = if inactive_days >= 365 {
                let years = (inactive_days as f64 / 365.0 * 10.0).round() / 10.0;
                (
                    "Dormant User File (1+ Year Inactive)",
                    format!(
                        "there has been no activity on this file since over 1 year ({} days / ~{:.1} yrs ago)",
                        inactive_days, years
                    ),
                    format!("{} · {}d inactive", size_str, inactive_days),
                )
            } else {
                (
                    "Dormant Large User File",
                    format!(
                        "there has been no activity on this file for {} days (approaching long-term 1-year dormancy)",
                        inactive_days.max(30)
                    ),
                    format!("{} · {}d inactive", size_str, inactive_days.max(30)),
                )
            };

            recommendations.push(WidgetRecommendation {
                id: format!("storage-dormant-{}", Self::hash_id(&item.path)),
                category: "storage".to_string(),
                sub_category: Some(sub_cat.to_string()),
                title: format!("Dormant User File: {}", item.name),
                message: format!(
                    "\"{}\" is occupying huge space ({}) and {}. Because this file is user-specific ({}), you should review it and archive or delete it if no longer needed.",
                    item.name, size_str, inactivity_phrase, item.path
                ),
                priority: if item.size >= 250 * 1024 * 1024 || inactive_days >= 365 {
                    "important".to_string()
                } else {
                    "interesting".to_string()
                },
                action_label: Some("Review User File in Explorer".to_string()),
                process_pid: None,
                process_name: None,
                startup_item_id: None,
                storage_path: Some(item.path.clone()),
                metric_highlight: Some(highlight),
                impact_level: Some(LoadImpactLevel::Moderate),
                sustained_seconds: None,
            });
        }

        // -------------------------------------------------------------------
        // Outcome B: Orphaned Application Files / Folders (Application not present on PC)
        // -------------------------------------------------------------------
        let orphan_candidates =
            Self::find_orphaned_app_items(&candidate_items, &installed_app_tokens, &mut seen_paths);

        for item in orphan_candidates.into_iter().take(3) {
            let size_str = Self::format_bytes(item.size);
            recommendations.push(WidgetRecommendation {
                id: format!("storage-orphan-{}", Self::hash_id(&item.path)),
                category: "storage".to_string(),
                sub_category: Some("Orphaned Application File".to_string()),
                title: format!("Uninstalled App Data: {}", item.name),
                message: format!(
                    "\"{}\" ({}) is an application {} occupying {}, but the corresponding application is not present on this PC. If you don't need this in the future, consider deleting it to reclaim storage.",
                    item.name,
                    item.path,
                    if item.is_dir { "data directory" } else { "package/file" },
                    size_str
                ),
                priority: if item.size >= 150 * 1024 * 1024 {
                    "important".to_string()
                } else {
                    "interesting".to_string()
                },
                action_label: Some("Consider Deleting Leftover App File".to_string()),
                process_pid: None,
                process_name: None,
                startup_item_id: None,
                storage_path: Some(item.path.clone()),
                metric_highlight: Some(format!("{} · App Missing", size_str)),
                impact_level: Some(LoadImpactLevel::Moderate),
                sustained_seconds: None,
            });
        }

        // -------------------------------------------------------------------
        // Outcome C: Reclaimable Cache & Temporary Data if additional insights helpful
        // -------------------------------------------------------------------
        for item in &candidate_items {
            if recommendations.len() >= 7 {
                break;
            }
            let norm_p = item.path.to_lowercase();
            if seen_paths.contains(&norm_p) {
                continue;
            }
            if (item.category == StorageCategory::CACHE
                || item.category == StorageCategory::TEMPORARY)
                && item.size >= 35 * 1024 * 1024
            {
                seen_paths.insert(norm_p);
                let size_str = Self::format_bytes(item.size);
                recommendations.push(WidgetRecommendation {
                    id: format!("storage-cache-{}", Self::hash_id(&item.path)),
                    category: "storage".to_string(),
                    sub_category: Some("Reclaimable Cache / Temp".to_string()),
                    title: format!("Large Temporary/Cache Data: {}", item.name),
                    message: format!(
                        "\"{}\" ({}) is occupying {} of temporary/cache storage. Clearing this directory when applications are closed will safely free disk space.",
                        item.name, item.path, size_str
                    ),
                    priority: "interesting".to_string(),
                    action_label: Some("Review Cache Directory".to_string()),
                    process_pid: None,
                    process_name: None,
                    startup_item_id: None,
                    storage_path: Some(item.path.clone()),
                    metric_highlight: Some(format!("{} Cache", size_str)),
                    impact_level: Some(LoadImpactLevel::Low),
                    sustained_seconds: None,
                });
            }
        }

        recommendations
    }

    fn collect_installed_app_tokens(running_procs: &[ProcessInfo]) -> HashSet<String> {
        let mut tokens = HashSet::new();

        // Include running process names
        for p in running_procs {
            let stem = p.name.trim_end_matches(".exe").to_lowercase();
            if stem.len() >= 3 {
                tokens.insert(stem);
            }
        }

        // Scan C:\Program Files, C:\Program Files (x86), and %LOCALAPPDATA%\Programs
        let mut dirs_to_check = vec![
            PathBuf::from(r"C:\Program Files"),
            PathBuf::from(r"C:\Program Files (x86)"),
        ];
        if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
            dirs_to_check.push(PathBuf::from(local_app).join("Programs"));
        }

        for dir in dirs_to_check {
            if let Ok(rd) = fs::read_dir(dir) {
                for entry in rd.flatten() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if name.len() >= 3 {
                        tokens.insert(name.clone());
                        for part in name.split(|c: char| !c.is_alphanumeric()) {
                            if part.len() >= 4 {
                                tokens.insert(part.to_string());
                            }
                        }
                    }
                }
            }
        }

        tokens
    }

    fn find_orphaned_app_items(
        candidates: &[StorageItem],
        installed_tokens: &HashSet<String>,
        seen_paths: &mut HashSet<String>,
    ) -> Vec<StorageItem> {
        let mut orphans = Vec::new();

        // Known system / common vendor names that should never be flagged as uninstalled apps
        let vendor_allowlist: HashSet<&str> = [
            "microsoft",
            "windows",
            "google",
            "mozilla",
            "apple",
            "nvidia",
            "intel",
            "amd",
            "realtek",
            "packages",
            "temp",
            "cache",
            "d3dscache",
            "connecteddevicesplatform",
            "identityservice",
            "comms",
            "publishers",
            "virtualstore",
            "programs",
            "squirreltemp",
            "npm",
            "npm-cache",
            "pnpm",
            "cargo",
            "rustup",
            "pip",
            "nuget",
            "code",
            "vscode",
            "git",
            "github",
            "tauri",
            "asao",
            "steam",
            "discord",
            "spotify",
            "onedrive",
        ]
        .into_iter()
        .collect();

        for item in candidates {
            if item.size < 8 * 1024 * 1024 {
                continue;
            }
            let norm_p = item.path.to_lowercase();
            if seen_paths.contains(&norm_p) {
                continue;
            }

            let is_appdata_folder = item.is_dir
                && (norm_p.contains("\\appdata\\local\\")
                    || norm_p.contains("\\appdata\\roaming\\")
                    || norm_p.contains(":\\programdata\\"));

            let is_standalone_installer = !item.is_dir
                && matches!(item.extension.as_str(), "exe" | "msi")
                && norm_p.contains("\\downloads\\");

            if !is_appdata_folder && !is_standalone_installer {
                continue;
            }

            let clean_name = item
                .name
                .trim_end_matches(".exe")
                .trim_end_matches(".msi")
                .trim_start_matches('.')
                .to_lowercase();

            if clean_name.len() < 3 || vendor_allowlist.contains(clean_name.as_str()) {
                continue;
            }

            // Check if any word in clean_name matches an installed application token
            let name_parts: Vec<&str> = clean_name
                .split(|c: char| !c.is_alphanumeric())
                .filter(|s| s.len() >= 4)
                .collect();

            let is_installed = installed_tokens.contains(&clean_name)
                || name_parts.iter().any(|part| {
                    vendor_allowlist.contains(part) || installed_tokens.contains(*part)
                });

            if !is_installed {
                seen_paths.insert(norm_p);
                orphans.push(item.clone());
            }
        }

        orphans.sort_by(|a, b| b.size.cmp(&a.size));
        orphans
    }

    fn fast_probe_user_and_app_storage() -> Vec<StorageItem> {
        let mut results = Vec::new();
        let Ok(user_profile) = std::env::var("USERPROFILE") else {
            return results;
        };
        let up = PathBuf::from(&user_profile);

        // 1. Probe user folders (Downloads, Documents, Videos, Desktop) for large dormant files
        let user_dirs = [
            (up.join("Downloads"), StorageCategory::USER),
            (up.join("Documents"), StorageCategory::USER),
            (up.join("Videos"), StorageCategory::USER),
            (up.join("Desktop"), StorageCategory::USER),
        ];

        for (dir, cat) in user_dirs {
            Self::probe_files_in_dir(&dir, cat, 2, &mut results);
        }

        // 2. Probe AppData\Roaming and AppData\Local top-level directories for orphaned app folders / caches
        let appdata_dirs = [up.join("AppData\\Roaming"), up.join("AppData\\Local")];
        for app_dir in appdata_dirs {
            let Ok(rd) = fs::read_dir(&app_dir) else {
                continue;
            };
            for entry in rd.flatten().take(45) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                let path_str = normalize_win_path(&path);
                let meta = entry.metadata().ok();
                let modified_ms = meta.as_ref().and_then(|m| system_time_to_ms(m.modified()));
                let created_ms = meta.as_ref().and_then(|m| system_time_to_ms(m.created()));
                let accessed_ms = meta.as_ref().and_then(|m| system_time_to_ms(m.accessed()));

                let size = compute_quick_dir_size(&path, 280);
                if size >= 8 * 1024 * 1024 {
                    let lower_n = name.to_lowercase();
                    let cat = if lower_n.contains("temp") {
                        StorageCategory::TEMPORARY
                    } else if lower_n.contains("cache") {
                        StorageCategory::CACHE
                    } else {
                        StorageCategory::APPLICATION
                    };
                    results.push(StorageItem {
                        name,
                        path: path_str,
                        is_dir: true,
                        size,
                        item_type: "Folder".to_string(),
                        extension: String::new(),
                        category: cat,
                        importance: crate::analyzer::storage_analyzer::StorageImportance::NORMAL,
                        created_ms,
                        modified_ms,
                        accessed_ms,
                        analysis_explanation: "Application data directory.".to_string(),
                    });
                }
            }
        }

        results
    }

    fn probe_files_in_dir(
        dir: &Path,
        category: StorageCategory,
        max_depth: u8,
        out: &mut Vec<StorageItem>,
    ) {
        if max_depth == 0 || out.len() > 200 {
            return;
        }
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten().take(80) {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_file() {
                let sz = meta.len();
                if sz >= 8 * 1024 * 1024 {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let ext = path
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    out.push(StorageItem {
                        name,
                        path: normalize_win_path(&path),
                        is_dir: false,
                        size: sz,
                        item_type: "File".to_string(),
                        extension: ext,
                        category,
                        importance: crate::analyzer::storage_analyzer::StorageImportance::NORMAL,
                        created_ms: system_time_to_ms(meta.created()),
                        modified_ms: system_time_to_ms(meta.modified()),
                        accessed_ms: system_time_to_ms(meta.accessed()),
                        analysis_explanation: "User file.".to_string(),
                    });
                }
            } else if meta.is_dir() && max_depth > 1 {
                Self::probe_files_in_dir(&path, category, max_depth - 1, out);
            }
        }
    }

    pub fn format_bytes(bytes: u64) -> String {
        let mb = bytes as f64 / (1024.0 * 1024.0);
        if mb >= 1024.0 {
            format!("{:.1} GB", mb / 1024.0)
        } else if mb >= 1.0 {
            format!("{:.0} MB", mb)
        } else {
            format!("{:.0} KB", bytes as f64 / 1024.0)
        }
    }

    fn hash_id(input: &str) -> String {
        let mut h: u64 = 14695981039346656037;
        for b in input.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(1099511628211);
        }
        format!("{:x}", h)
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
