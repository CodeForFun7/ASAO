//! Tauri commands for residual file cleanup (Storage, Leftovers tab).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::AppMonitoringState;
use crate::db::ScanKind;
use crate::residual::cleaner::{freed_bytes, process_items, CleanItem, CleanOptions, CleanOutcome, CleanReport};
use crate::residual::elevated::{is_process_elevated, run_elevated, ElevatedJob, ElevationError};
use crate::residual::engine::{build_live_context, refresh_history, Engine, LiveOptions};
use crate::residual::normalize::path_key;
use crate::residual::sizer;
use crate::residual::store_files::{write_scan_log, IgnoreList, ResidualFiles};
use crate::residual::{
    dev_mode, now_ms, LeftoverSummary, ResidualScanProgress, ResidualScanResult, ResidualSizeUpdate,
    ResidualSizingDone, EVENT_RESIDUAL_SCAN_PROGRESS, EVENT_RESIDUAL_SIZE_UPDATE,
    EVENT_RESIDUAL_SIZING_DONE,
};

#[derive(Default)]
pub struct ResidualRuntime {
    pub last_result: Option<ResidualScanResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualInfo {
    pub dev_mode: bool,
    pub is_elevated: bool,
    pub data_dir: String,
    pub deletion_log: String,
    pub dry_run_log: String,
    pub scan_log: String,
}

struct BusyGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub fn residual_files(app: &AppHandle) -> Result<ResidualFiles, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("App data folder unavailable: {}", e))?;
    Ok(ResidualFiles::new(&dir))
}

pub fn protected_paths(app: &AppHandle) -> Vec<String> {
    let mut out = Vec::new();
    for dir in [
        app.path().app_data_dir().ok(),
        app.path().app_local_data_dir().ok(),
        app.path().app_cache_dir().ok(),
        app.path().app_config_dir().ok(),
    ]
    .into_iter()
    .flatten()
    {
        out.push(dir.to_string_lossy().to_string());
    }
    out
}

pub fn running_exes(state: &AppMonitoringState) -> Vec<String> {
    state
        .engine
        .lock()
        .ok()
        .and_then(|mut g| g.get_latest_or_collect().ok())
        .map(|s| {
            s.processes
                .into_iter()
                .filter_map(|p| p.executable_path)
                .map(|p| path_key(&p))
                .collect()
        })
        .unwrap_or_default()
}

fn min_age_days(state: &AppMonitoringState) -> u32 {
    state
        .settings
        .lock()
        .map(|s| s.residual_min_age_days)
        .unwrap_or(crate::residual::DEFAULT_MIN_AGE_DAYS)
}

/// Leftover totals for the Storage suggestion, from the latest scan.
pub fn leftover_summary(state: &AppMonitoringState) -> Option<LeftoverSummary> {
    let guard = state.residual.lock().ok()?;
    let result = guard.last_result.as_ref()?;
    let summary = result.confident_summary();
    if summary.folder_count == 0 {
        None
    } else {
        Some(summary)
    }
}

/// Recomputes the cached storage suggestions so the new leftover totals show up.
pub fn refresh_storage_recommendations(state: &AppMonitoringState) {
    let summary = leftover_summary(state);
    if let Ok(st_guard) = state.storage_engine.lock() {
        let recs = crate::analyzer::recommendation_engine::RecommendationEngine::evaluate_storage_recommendations(
            &st_guard,
            summary.as_ref(),
        );
        if let Ok(mut guard) = state.cached_storage_recs.lock() {
            *guard = recs;
        }
    }
}

/// Runs a full scan, stores the result and starts background sizing.
pub fn run_scan_blocking(app: &AppHandle, state: &AppMonitoringState) -> Result<ResidualScanResult, String> {
    if state
        .residual_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A leftover scan or cleanup is already running.".into());
    }
    let _busy = BusyGuard(&state.residual_busy);
    state.residual_cancel.store(false, Ordering::SeqCst);

    let files = residual_files(app)?;
    let _ = app.emit(
        EVENT_RESIDUAL_SCAN_PROGRESS,
        ResidualScanProgress {
            phase: "inventory".into(),
            current_path: "Reading installed apps, services and scheduled tasks".into(),
            folders_checked: 0,
            candidates_found: 0,
        },
    );
    let live = build_live_context(
        &files,
        LiveOptions {
            min_age_days: min_age_days(state),
            running_exes: running_exes(state),
            protected_paths: protected_paths(app),
            update_history: true,
            only_roots: None,
        },
    )?;
    let engine = Engine::new(&live.ctx);

    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut result = engine.scan(&state.residual_cancel, &mut |p| {
        if last_emit.elapsed() >= Duration::from_millis(80) {
            let _ = app.emit(EVENT_RESIDUAL_SCAN_PROGRESS, p.clone());
            last_emit = Instant::now();
        }
    })?;

    if let Some(obs) = &live.observe {
        result.log.push(format!(
            "App history: {} new, {} newly removed, {} reinstalled{}",
            obs.newly_seen,
            obs.newly_removed,
            obs.reinstalled,
            if obs.guarded_classes.is_empty() {
                String::new()
            } else {
                format!(" (removals not recorded for: {})", obs.guarded_classes.join(", "))
            }
        ));
    }
    if !result.stats.references_available {
        result.log.push(
            "Could not read services and scheduled tasks. Folders can still be reviewed, but removal will be refused until this works.".into(),
        );
    }
    write_scan_log(&files.scan_log(), &result.log);

    if let Ok(mut guard) = state.residual.lock() {
        guard.last_result = Some(result.clone());
    }
    let run_id = state.cache_run(
        ScanKind::Leftover,
        "",
        result.started_ms,
        result.finished_ms.saturating_sub(result.started_ms),
        result.candidates().count() as u64,
        &result,
    );
    start_sizing(app.clone(), state.clone(), result.clone(), run_id);
    Ok(result)
}

/// Measures every flagged folder in the background and streams the sizes to the UI.
fn start_sizing(app: AppHandle, state: AppMonitoringState, result: ResidualScanResult, run_id: Option<i64>) {
    std::thread::spawn(move || {
        let scan_id = result.scan_id;
        let mut total = 0u64;
        let mut items: Vec<_> = result.candidates().cloned().collect();
        items.sort_by(|a, b| b.confidence.cmp(&a.confidence));
        let never = std::sync::atomic::AtomicBool::new(false);
        for c in items {
            // Stop if a newer scan replaced this one.
            let current = state
                .residual
                .lock()
                .ok()
                .and_then(|g| g.last_result.as_ref().map(|r| r.scan_id));
            if current != Some(scan_id) {
                return;
            }
            let size = sizer::measure(&PathBuf::from(&c.path), &never);
            total = total.saturating_add(size.size_on_disk);
            if let Ok(mut guard) = state.residual.lock() {
                if let Some(r) = guard.last_result.as_mut() {
                    if r.scan_id == scan_id {
                        r.set_size(&c.path, size.size_on_disk, size.newest_modified_ms);
                    }
                }
            }
            let _ = app.emit(
                EVENT_RESIDUAL_SIZE_UPDATE,
                ResidualSizeUpdate {
                    scan_id,
                    path: c.path.clone(),
                    size_bytes: size.size_on_disk,
                    newest_modified_ms: size.newest_modified_ms,
                },
            );
        }
        // Save the measured sizes into the cached run (only if this scan is still current).
        if let Ok(guard) = state.residual.lock() {
            if let Some(r) = guard.last_result.as_ref().filter(|r| r.scan_id == scan_id) {
                state.cache_update(run_id, r.candidates().count() as u64, r);
            }
        }
        let _ = app.emit(
            EVENT_RESIDUAL_SIZING_DONE,
            ResidualSizingDone {
                scan_id,
                total_bytes: total,
            },
        );
        refresh_storage_recommendations(&state);
    });
}

/// Quiet work at launch: record the app history, then run one background scan so the
/// Storage suggestion has numbers. Nothing is ever deleted from here.
pub fn start_background_work(app: AppHandle, state: AppMonitoringState) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        if let Ok(files) = residual_files(&app) {
            let _ = refresh_history(&files);
        }
        std::thread::sleep(Duration::from_secs(45));
        let has_result = state
            .residual
            .lock()
            .map(|g| g.last_result.is_some())
            .unwrap_or(true);
        if !has_result {
            let _ = run_scan_blocking(&app, &state);
        }
    });
}

#[tauri::command]
pub async fn residual_scan(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
) -> Result<ResidualScanResult, String> {
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || run_scan_blocking(&app, &st))
        .await
        .map_err(|e| format!("Scan thread error: {}", e))?
}

#[tauri::command]
pub fn residual_cancel_scan(state: State<'_, AppMonitoringState>) -> Result<(), String> {
    state.residual_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn residual_get_last_result(
    state: State<'_, AppMonitoringState>,
) -> Result<Option<ResidualScanResult>, String> {
    let guard = state
        .residual
        .lock()
        .map_err(|_| "Leftover state is unavailable".to_string())?;
    Ok(guard.last_result.clone())
}

#[tauri::command]
pub async fn residual_clean(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
    paths: Vec<String>,
    dry_run: bool,
    permanent: bool,
) -> Result<CleanReport, String> {
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || clean_blocking(&app, &st, paths, dry_run, permanent))
        .await
        .map_err(|e| format!("Cleanup thread error: {}", e))?
}

fn clean_blocking(
    app: &AppHandle,
    state: &AppMonitoringState,
    paths: Vec<String>,
    dry_run: bool,
    permanent: bool,
) -> Result<CleanReport, String> {
    let files = residual_files(app)?;
    let log_path = if dry_run { files.dry_run_log() } else { files.deletion_log() };

    // Only folders from the latest scan can be removed.
    let mut outcomes: Vec<CleanOutcome> = Vec::new();
    let mut planned: Vec<CleanItem> = Vec::new();
    {
        let guard = state
            .residual
            .lock()
            .map_err(|_| "Leftover state is unavailable".to_string())?;
        let Some(result) = guard.last_result.as_ref() else {
            return Err("Run a scan first.".into());
        };
        for p in &paths {
            match result.find_candidate(p) {
                Some(c) => planned.push(CleanItem::from(c)),
                None => outcomes.push(CleanOutcome::skipped(
                    &CleanItem {
                        path: p.clone(),
                        name: p.rsplit('\\').next().unwrap_or(p).to_string(),
                        category: None,
                        size_bytes: None,
                        needs_admin: false,
                        in_onedrive: false,
                    },
                    "Not part of the latest scan",
                )),
            }
        }
    }

    if state
        .residual_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A leftover scan or cleanup is already running.".into());
    }
    let _busy = BusyGuard(&state.residual_busy);

    let min_age = min_age_days(state);
    let running = running_exes(state);
    let protected = protected_paths(app);
    let opts = CleanOptions { dry_run, permanent, only_empty: false };
    let elevated = is_process_elevated();
    let mut admin_requested = false;
    let mut admin_declined = false;

    let live = build_live_context(
        &files,
        LiveOptions {
            min_age_days: min_age,
            running_exes: running.clone(),
            protected_paths: protected.clone(),
            update_history: false,
            only_roots: None,
        },
    );
    match live {
        Err(e) => {
            outcomes.extend(planned.iter().map(|i| CleanOutcome::skipped(i, e.clone())));
        }
        Ok(live) => {
            let engine = Engine::new(&live.ctx);
            let (admin_items, user_items): (Vec<CleanItem>, Vec<CleanItem>) =
                planned.into_iter().partition(|i| i.needs_admin);

            outcomes.extend(process_items(&engine, &user_items, opts, &log_path, elevated));

            if !admin_items.is_empty() {
                if dry_run || elevated {
                    outcomes.extend(process_items(&engine, &admin_items, opts, &log_path, elevated));
                } else {
                    admin_requested = true;
                    let app_data_dir = files
                        .dir
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let job = ElevatedJob {
                        items: admin_items.clone(),
                        permanent,
                        app_data_dir,
                        min_age_days: min_age,
                        running_exes: running,
                        protected_paths: protected,
                        result_path: String::new(),
                    };
                    match run_elevated(job, &files) {
                        Ok(outs) => outcomes.extend(outs),
                        Err(ElevationError::Declined) => {
                            admin_declined = true;
                            outcomes.extend(admin_items.iter().map(|i| {
                                CleanOutcome::skipped(i, "Administrator permission was declined")
                            }));
                        }
                        Err(ElevationError::Failed(e)) => {
                            outcomes.extend(admin_items.iter().map(|i| CleanOutcome::skipped(i, e.clone())));
                        }
                    }
                }
            }
        }
    }

    if !dry_run {
        let removed: Vec<String> = outcomes
            .iter()
            .filter(|o| o.status == crate::residual::cleaner::CleanStatus::Deleted)
            .map(|o| o.path.clone())
            .collect();
        if let Ok(mut guard) = state.residual.lock() {
            if let Some(r) = guard.last_result.as_mut() {
                r.remove_paths(&removed);
            }
        }
        refresh_storage_recommendations(state);
    }

    Ok(CleanReport {
        dry_run,
        permanent,
        freed_bytes: freed_bytes(&outcomes),
        outcomes,
        admin_requested,
        admin_declined,
        log_path: log_path.to_string_lossy().to_string(),
    })
}

/// Moves the hidden empty folders of the latest scan to the Recycle Bin. Each one is
/// checked again first and must still hold no files. Only folders in your own AppData.
#[tauri::command]
pub async fn residual_clean_empty(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
    dry_run: bool,
) -> Result<CleanReport, String> {
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || clean_empty_blocking(&app, &st, dry_run))
        .await
        .map_err(|e| format!("Cleanup thread error: {}", e))?
}

fn clean_empty_blocking(app: &AppHandle, state: &AppMonitoringState, dry_run: bool) -> Result<CleanReport, String> {
    let files = residual_files(app)?;
    let log_path = if dry_run { files.dry_run_log() } else { files.deletion_log() };
    let items: Vec<CleanItem> = {
        let guard = state
            .residual
            .lock()
            .map_err(|_| "Leftover state is unavailable".to_string())?;
        let Some(result) = guard.last_result.as_ref() else {
            return Err("Run a scan first.".into());
        };
        result
            .empty_folders
            .iter()
            .map(|p| CleanItem {
                path: p.clone(),
                name: p.rsplit('\\').next().unwrap_or(p).to_string(),
                category: None,
                size_bytes: Some(0),
                needs_admin: false,
                in_onedrive: false,
            })
            .collect()
    };
    if items.is_empty() {
        return Err("There are no empty folders to clean.".into());
    }

    if state
        .residual_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A leftover scan or cleanup is already running.".into());
    }
    let _busy = BusyGuard(&state.residual_busy);

    let live = build_live_context(
        &files,
        LiveOptions {
            min_age_days: min_age_days(state),
            running_exes: running_exes(state),
            protected_paths: protected_paths(app),
            update_history: false,
            only_roots: None,
        },
    )?;
    let engine = Engine::new(&live.ctx);
    let opts = CleanOptions { dry_run, permanent: false, only_empty: true };
    let outcomes = process_items(&engine, &items, opts, &log_path, is_process_elevated());

    if !dry_run {
        let removed: Vec<String> = outcomes
            .iter()
            .filter(|o| o.status == crate::residual::cleaner::CleanStatus::Deleted)
            .map(|o| o.path.clone())
            .collect();
        if let Ok(mut guard) = state.residual.lock() {
            if let Some(r) = guard.last_result.as_mut() {
                r.remove_empty_folders(&removed);
            }
        }
    }

    Ok(CleanReport {
        dry_run,
        permanent: false,
        freed_bytes: 0,
        outcomes,
        admin_requested: false,
        admin_declined: false,
        log_path: log_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn residual_get_ignore_list(app: AppHandle) -> Result<IgnoreList, String> {
    let files = residual_files(&app)?;
    Ok(IgnoreList::load(&files.ignore_list()))
}

#[tauri::command]
pub fn residual_ignore_folder(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
    path: String,
) -> Result<IgnoreList, String> {
    let files = residual_files(&app)?;
    let mut list = IgnoreList::load(&files.ignore_list());
    list.add_folder(&path, now_ms());
    list.save(&files.ignore_list())?;
    if let Ok(mut guard) = state.residual.lock() {
        if let Some(r) = guard.last_result.as_mut() {
            r.remove_paths(&[path]);
        }
    }
    refresh_storage_recommendations(&state);
    Ok(list)
}

#[tauri::command]
pub fn residual_ignore_app(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
    key: String,
    name: String,
) -> Result<IgnoreList, String> {
    if key.starts_with("unknown") {
        return Err("Unknown leftovers can only be ignored one folder at a time.".into());
    }
    let files = residual_files(&app)?;
    let mut list = IgnoreList::load(&files.ignore_list());
    list.add_app(&key, &name, now_ms());
    list.save(&files.ignore_list())?;
    if let Ok(mut guard) = state.residual.lock() {
        if let Some(r) = guard.last_result.as_mut() {
            r.remove_app_group(&key);
        }
    }
    refresh_storage_recommendations(&state);
    Ok(list)
}

#[tauri::command]
pub fn residual_unignore_folder(app: AppHandle, path: String) -> Result<IgnoreList, String> {
    let files = residual_files(&app)?;
    let mut list = IgnoreList::load(&files.ignore_list());
    list.remove_folder(&path);
    list.save(&files.ignore_list())?;
    Ok(list)
}

#[tauri::command]
pub fn residual_unignore_app(app: AppHandle, key: String) -> Result<IgnoreList, String> {
    let files = residual_files(&app)?;
    let mut list = IgnoreList::load(&files.ignore_list());
    list.remove_app(&key);
    list.save(&files.ignore_list())?;
    Ok(list)
}

#[tauri::command]
pub fn residual_open_recycle_bin() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg("shell:RecycleBinFolder")
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open the Recycle Bin: {}", e))
}

#[tauri::command]
pub fn residual_get_info(app: AppHandle) -> Result<ResidualInfo, String> {
    let files = residual_files(&app)?;
    Ok(ResidualInfo {
        dev_mode: dev_mode(),
        is_elevated: is_process_elevated(),
        data_dir: files.dir.to_string_lossy().to_string(),
        deletion_log: files.deletion_log().to_string_lossy().to_string(),
        dry_run_log: files.dry_run_log().to_string_lossy().to_string(),
        scan_log: files.scan_log().to_string_lossy().to_string(),
    })
}
