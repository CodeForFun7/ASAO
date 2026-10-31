//! Tauri commands for the Debloat page. The scan is read-only: it never uninstalls,
//! stops or disables anything.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::apps::usage::{last_use_in, prefetch_last_runs};
use crate::bloat::database::BloatwareDatabase;
use crate::bloat::elevated::{
    action_log, dry_run_log, history_path, run_batch, run_job_elevated, run_undo_here, ApplyReport, BloatJob,
    JobMode, UndoResult,
};
use crate::bloat::executor::{RestorePointPolicy, RestorePointResult, RestorePointStatus};
use crate::bloat::history::{undo_for, ChangeHistory};
use crate::bloat::planner::{plan_with_tweaks, Plan, PlanOptions};
use crate::bloat::w11d::TweakSet;
use crate::bloat::{dry_run_forced, effective_dry_run, EVENT_BLOAT_APPLY_PROGRESS};
use crate::residual::elevated::{is_process_elevated, ElevationError};
use crate::bloat::impact::{read_startup_info, sample};
use crate::bloat::inventory::{collect_device, collect_services, collect_startup, collect_tasks};
use crate::bloat::scan::ScanInputs;
use crate::bloat::thresholds::SAMPLE_SECONDS;
use crate::bloat::{BloatScanProgress, BloatScanResult, BloatScanStats, EVENT_BLOAT_SCAN_PROGRESS};
use crate::commands::apps::{ensure_usage_loaded, list_exes, places};
use crate::commands::AppMonitoringState;
use crate::residual::evidence::userassist_records;
use crate::residual::inventory::{collect_installed_apps, AppSource};
use crate::residual::sizer;
use crate::residual::{dev_mode, now_ms};

#[derive(Default)]
pub struct BloatRuntime {
    pub last_result: Option<BloatScanResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatInfo {
    pub dev_mode: bool,
    pub data_dir: String,
    /// Put an edited `bloatware-db.json` here to replace the built-in list.
    pub db_override_path: String,
    pub db_override_active: bool,
    pub db_version: u32,
    pub db_updated: String,
    pub db_entries: usize,
    pub db_problem: Option<String>,
    /// Dry run is on by default (development builds, or forced).
    pub dry_run_default: bool,
    /// The dry run switch is shown (development builds only).
    pub dry_run_toggle_visible: bool,
    /// Forced on by the setting, `--bloat-dry-run` or `ASAO_BLOAT_DRY_RUN=1`.
    pub dry_run_forced: bool,
    pub action_log: String,
    pub dry_run_log: String,
    pub history_file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
}

fn forced_dry_run(state: &AppMonitoringState) -> bool {
    dry_run_forced(state.settings.lock().map(|s| s.bloat_dry_run).unwrap_or(false))
}

struct BusyGuard<'a>(&'a AtomicBool);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub fn bloat_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("bloat"))
        .map_err(|e| format!("App data folder unavailable: {}", e))
}

fn db_override_path(dir: &Path) -> PathBuf {
    dir.join("bloatware-db.json")
}

fn emit(app: &AppHandle, phase: &str, message: impl Into<String>, percent: Option<u8>) {
    let _ = app.emit(
        EVENT_BLOAT_SCAN_PROGRESS,
        BloatScanProgress {
            phase: phase.into(),
            message: message.into(),
            percent,
        },
    );
}

fn scan_blocking(app: &AppHandle, state: &AppMonitoringState) -> Result<BloatScanResult, String> {
    if state
        .bloat_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A bloatware scan is already running.".into());
    }
    let _busy = BusyGuard(&state.bloat_busy);
    state.bloat_cancel.store(false, Ordering::SeqCst);
    let started = now_ms();
    let dir = bloat_dir(app)?;
    let mut log = Vec::new();

    let (db, w11d, db_source, problems) = BloatwareDatabase::load_combined(&dir);
    log.extend(problems);

    emit(app, "inventory", "Reading installed apps", None);
    let inventory = collect_installed_apps();
    if inventory.failed_classes.contains(&"registry") {
        return Err("Could not read the list of installed apps.".into());
    }
    for class in &inventory.failed_classes {
        log.push(format!("Could not read {} apps", class));
    }
    emit(app, "inventory", "Reading services and scheduled tasks", None);
    let services = collect_services().unwrap_or_else(|| {
        log.push("Could not read the list of services".into());
        Vec::new()
    });
    let tasks = collect_tasks().unwrap_or_default();
    let processes = || {
        state
            .engine
            .lock()
            .ok()
            .and_then(|mut g| g.get_latest_or_collect().ok())
            .map(|s| s.processes)
            .unwrap_or_default()
    };
    emit(app, "inventory", "Reading startup entries", None);
    let startup = collect_startup(&processes());
    let has_gaming_services = inventory.apps.iter().any(|a| {
        a.source == AppSource::Store
            && a.package_name
                .as_deref()
                .map(|p| p.eq_ignore_ascii_case("Microsoft.GamingServices"))
                .unwrap_or(false)
    });
    emit(app, "inventory", "Checking this PC", None);
    let mut device = collect_device(has_gaming_services);
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    let startup_info = read_startup_info(&Path::new(&windir).join(r"System32\WDI\LogFiles\StartupInfo"));
    device.startup_measured = startup_info.is_some();
    emit(app, "inventory", "Checking Windows settings", None);
    let tweaks = w11d.tweaks(device.windows_build, true);

    ensure_usage_loaded(app, state);
    let usage = state.apps.lock().map(|rt| rt.usage.clone()).unwrap_or_default();
    let runs = userassist_records();
    let prefetch = prefetch_last_runs();

    // Background CPU and memory, sampled without blocking the UI.
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let samples = sample(SAMPLE_SECONDS, &processes, &state.bloat_cancel, &mut |pct| {
        if last_emit.elapsed() >= Duration::from_millis(400) {
            emit(app, "sampling", "Measuring background CPU and memory use", Some(pct));
            last_emit = Instant::now();
        }
    });
    if state.bloat_cancel.load(Ordering::SeqCst) {
        return Err("Scan cancelled.".into());
    }

    emit(app, "scoring", "Scoring impact and safety", None);
    let p = places();
    let mut broad = p.broad.clone();
    broad.extend(p.roots.iter().cloned());
    let last_use = |dirs: &[String], family: Option<&str>| {
        last_use_in(&runs, &usage, prefetch.as_ref(), &list_exes, dirs, family).map(|b| b.0)
    };
    let never = AtomicBool::new(false);
    let folder_size = |dir: &str| Some(sizer::measure(Path::new(dir), &never).size_on_disk);
    let items = ScanInputs {
        db: &db,
        apps: &inventory.apps,
        details: &inventory.details,
        services: &services,
        startup: &startup,
        tasks: &tasks,
        device: &device,
        samples: &samples,
        startup_info: startup_info.as_ref(),
        broad: &broad,
        windir: p.windir.as_deref(),
        last_use: &last_use,
        folder_size: &folder_size,
        now_ms: now_ms(),
    }
    .build();

    let result = BloatScanResult {
        scan_id: started,
        started_ms: started,
        finished_ms: now_ms(),
        stats: BloatScanStats {
            apps: inventory.apps.len(),
            services: services.len(),
            startup_entries: startup.len(),
            tasks: tasks.len(),
            sample_seconds: samples.seconds,
        },
        items,
        device,
        db_version: db.version,
        db_updated: db.updated.clone(),
        db_source,
        tweaks,
        presets: w11d.presets.clone(),
        w11d_source: w11d.source.clone(),
        log,
    };
    if let Ok(mut rt) = state.bloat.lock() {
        rt.last_result = Some(result.clone());
    }
    Ok(result)
}

#[tauri::command]
pub async fn bloat_scan(app: AppHandle, state: State<'_, AppMonitoringState>) -> Result<BloatScanResult, String> {
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || scan_blocking(&app, &st))
        .await
        .map_err(|e| format!("Scan thread error: {}", e))?
}

#[tauri::command]
pub fn bloat_cancel_scan(state: State<'_, AppMonitoringState>) -> Result<(), String> {
    state.bloat_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn bloat_get_last_result(state: State<'_, AppMonitoringState>) -> Result<Option<BloatScanResult>, String> {
    state
        .bloat
        .lock()
        .map(|rt| rt.last_result.clone())
        .map_err(|_| "Bloatware state is unavailable".to_string())
}

#[tauri::command]
pub fn bloat_get_info(app: AppHandle) -> Result<BloatInfo, String> {
    let dir = bloat_dir(&app)?;
    let override_path = db_override_path(&dir);
    let (db, _w11d, source, problems) = BloatwareDatabase::load_combined(&dir);
    let problem = (!problems.is_empty()).then(|| problems.join(" "));
    let state = app.state::<AppMonitoringState>();
    let forced = forced_dry_run(&state);
    Ok(BloatInfo {
        dry_run_default: dev_mode() || forced,
        dry_run_toggle_visible: dev_mode() && !forced,
        dry_run_forced: forced,
        action_log: action_log(&dir).to_string_lossy().to_string(),
        dry_run_log: dry_run_log(&dir).to_string_lossy().to_string(),
        history_file: history_path(&dir).to_string_lossy().to_string(),
        dev_mode: dev_mode(),
        data_dir: dir.to_string_lossy().to_string(),
        db_override_active: source != "Built in" || _w11d.source != "Built in",
        db_override_path: override_path.to_string_lossy().to_string(),
        db_version: db.version,
        db_updated: db.updated.clone(),
        db_entries: db.entries.len(),
        db_problem: problem,
    })
}

fn last_scan(state: &AppMonitoringState) -> Result<(Vec<crate::bloat::BloatItem>, TweakSet), String> {
    state
        .bloat
        .lock()
        .map_err(|_| "Bloatware state is unavailable".to_string())?
        .last_result
        .as_ref()
        .map(|r| (r.items.clone(), r.tweaks.clone()))
        .ok_or_else(|| "Run a scan first.".to_string())
}

/// What a batch would do, for the confirm dialog. Changes nothing.
#[tauri::command]
pub fn bloat_plan(
    state: State<'_, AppMonitoringState>,
    selected_ids: Vec<String>,
    selected_tweaks: Vec<String>,
    remove_provisioned: bool,
) -> Result<Plan, String> {
    let (items, tweaks) = last_scan(&state)?;
    Ok(plan_with_tweaks(&items, &selected_ids, &tweaks, &selected_tweaks, &PlanOptions { remove_provisioned }))
}

fn apply_blocking(
    app: &AppHandle,
    state: &AppMonitoringState,
    selected_ids: Vec<String>,
    selected_tweaks: Vec<String>,
    dry_run_requested: bool,
    remove_provisioned: bool,
    continue_without_restore_point: bool,
) -> Result<ApplyReport, String> {
    let (items, tweaks) = last_scan(state)?;
    if state
        .bloat_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A bloatware scan or change is already running.".into());
    }
    let _busy = BusyGuard(&state.bloat_busy);
    let dir = bloat_dir(app)?;
    let dry_run = effective_dry_run(dry_run_requested, dev_mode(), forced_dry_run(state));
    let options = PlanOptions { remove_provisioned };
    let policy = if continue_without_restore_point { RestorePointPolicy::Continue } else { RestorePointPolicy::Stop };
    let batch_id = now_ms().to_string();

    let report = if dry_run || is_process_elevated() {
        let (_, w11d, _, _) = BloatwareDatabase::load_combined(&dir);
        run_batch(&items, &selected_ids, &w11d, &tweaks, &selected_tweaks, &options, policy, dry_run, &dir, &batch_id, &mut |done, total, current| {
            let _ = app.emit(EVENT_BLOAT_APPLY_PROGRESS, ApplyProgress { done, total, current: current.to_string() });
        })
    } else {
        // Only items ASAO itself would act on go into the job; the helper re-checks them.
        let planned = plan_with_tweaks(&items, &selected_ids, &tweaks, &selected_tweaks, &options);
        let allowed: Vec<String> = {
            let mut ids: Vec<String> = planned
                .actions
                .iter()
                .map(|a| a.item_id.clone())
                .filter(|i| !i.is_empty() && !i.starts_with("tweak:"))
                .collect();
            ids.dedup();
            ids
        };
        let allowed_tweaks: Vec<String> = planned
            .actions
            .iter()
            .filter_map(|a| match &a.kind {
                crate::bloat::planner::ActionKind::ApplyTweak { feature_id, .. } => Some(feature_id.clone()),
                _ => None,
            })
            .collect();
        let _ = app.emit(
            EVENT_BLOAT_APPLY_PROGRESS,
            ApplyProgress { done: 0, total: planned.actions.len(), current: "Waiting for administrator permission".into() },
        );
        let mut job = BloatJob {
            mode: JobMode::Apply { selected_ids: allowed, selected_tweaks: allowed_tweaks, options, restore_point_policy: policy },
            bloat_dir: dir.to_string_lossy().to_string(),
            batch_id: batch_id.clone(),
            result_path: String::new(),
        };
        let nothing = |declined: bool, error: Option<String>| ApplyReport {
            batch_id: batch_id.clone(),
            dry_run: false,
            restore_point: RestorePointResult { status: RestorePointStatus::Skipped, message: "Nothing was changed.".into() },
            outcomes: Vec::new(),
            rejected: planned.rejected.clone(),
            stopped: true,
            admin_requested: true,
            admin_declined: declined,
            log_path: action_log(&dir).to_string_lossy().to_string(),
            uninstalled: Vec::new(),
            settings_changed: false,
            restart_needed: false,
            error,
        };
        match run_job_elevated::<ApplyReport>(&mut job, &dir) {
            Ok(mut r) => {
                r.admin_requested = true;
                r.rejected.extend(planned.rejected.clone());
                r
            }
            Err(ElevationError::Declined) => nothing(true, Some("Administrator permission was declined. Nothing was changed.".into())),
            Err(ElevationError::Failed(e)) => nothing(false, Some(e)),
        }
    };

    // Uninstalled apps leave the list; the page scans again for the rest.
    if !report.dry_run {
        let gone: Vec<String> = report
            .outcomes
            .iter()
            .filter(|o| o.status == crate::bloat::executor::OutcomeStatus::Done)
            .filter(|o| matches!(o.action.kind, crate::bloat::planner::ActionKind::UninstallStore { .. } | crate::bloat::planner::ActionKind::UninstallWin32 { .. }))
            .map(|o| o.action.item_id.clone())
            .collect();
        if let Ok(mut rt) = state.bloat.lock() {
            if let Some(r) = rt.last_result.as_mut() {
                r.items.retain(|i| !gone.contains(&i.id));
            }
        }
    }
    Ok(report)
}

/// Runs the ticked changes: restore point, startup entries, tasks, services, uninstalls.
#[tauri::command]
pub async fn bloat_apply(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
    selected_ids: Vec<String>,
    selected_tweaks: Vec<String>,
    dry_run: bool,
    remove_provisioned: bool,
    continue_without_restore_point: bool,
) -> Result<ApplyReport, String> {
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        apply_blocking(&app, &st, selected_ids, selected_tweaks, dry_run, remove_provisioned, continue_without_restore_point)
    })
    .await
    .map_err(|e| format!("Thread error: {}", e))?
}

#[tauri::command]
pub fn bloat_history(app: AppHandle) -> Result<ChangeHistory, String> {
    Ok(ChangeHistory::load(&history_path(&bloat_dir(&app)?)))
}

fn undo_blocking(app: &AppHandle, batch_id: String, action_id: String) -> Result<UndoResult, String> {
    let dir = bloat_dir(app)?;
    let history = ChangeHistory::load(&history_path(&dir));
    let record = history
        .find(&batch_id, &action_id)
        .ok_or_else(|| "That change is not in Recent changes.".to_string())?;
    if let Some(url) = &record.store_url {
        crate::apps::actions::shell_open(url, None).map_err(|e| match e {
            crate::apps::actions::LaunchError::Declined => "Cancelled".to_string(),
            crate::apps::actions::LaunchError::Failed(m) => m,
        })?;
        return Ok(UndoResult { ok: true, message: "Opened its Microsoft Store page.".into() });
    }
    let Some(kind) = undo_for(&record.outcome) else {
        return Ok(UndoResult { ok: false, message: record.undo_hint.clone() });
    };
    if kind.needs_admin() && !is_process_elevated() {
        let mut job = BloatJob {
            mode: JobMode::Undo { batch_id: batch_id.clone(), action_id: action_id.clone() },
            bloat_dir: dir.to_string_lossy().to_string(),
            batch_id: format!("undo-{}", now_ms()),
            result_path: String::new(),
        };
        return match run_job_elevated::<UndoResult>(&mut job, &dir) {
            Ok(r) => Ok(r),
            Err(ElevationError::Declined) => Ok(UndoResult { ok: false, message: "Administrator permission was declined.".into() }),
            Err(ElevationError::Failed(e)) => Ok(UndoResult { ok: false, message: e }),
        };
    }
    Ok(run_undo_here(&dir, &batch_id, &action_id))
}

/// Puts one recorded change back. Store apps: opens their Store page.
#[tauri::command]
pub async fn bloat_undo(app: AppHandle, batch_id: String, action_id: String) -> Result<UndoResult, String> {
    tauri::async_runtime::spawn_blocking(move || undo_blocking(&app, batch_id, action_id))
        .await
        .map_err(|e| format!("Thread error: {}", e))?
}

/// Restarts File Explorer so taskbar, Start and File Explorer settings show at once, as
/// Win11Debloat does. Open File Explorer windows close. Runs as you, not as administrator.
#[tauri::command]
pub fn bloat_restart_explorer() -> Result<(), String> {
    crate::bloat::executor::ps("Stop-Process -Name explorer -Force; Start-Sleep -Milliseconds 1500; if (-not (Get-Process explorer -ErrorAction SilentlyContinue)) { Start-Process explorer.exe }")
        .map(|_| ())
}
