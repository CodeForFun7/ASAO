//! Running a batch: in ASAO itself (dry run, or when ASAO already runs as administrator), or
//! in a second copy of `asao.exe` started as administrator (one Windows prompt per batch).
//!
//! The administrator helper does not trust its job file for what to change: it rescans the
//! PC, rates everything again and plans from that, so only items still rated Safe to remove
//! or Usually safe can be touched. Undo jobs are checked against the recorded history.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::database::BloatwareDatabase;
use super::w11d::{TweakSet, Win11Debloat};
use super::executor::{execute, ExecOptions, ExecReport, OutcomeStatus, RestorePointPolicy};
use super::history::{run_undo, undo_for, ChangeHistory};
use super::impact::Samples;
use super::inventory::{collect_device, collect_services, collect_startup, collect_tasks};
use super::planner::{plan_with_tweaks, ActionKind, PlanOptions, Rejected};
use super::scan::ScanInputs;
use super::BloatItem;
use crate::residual::elevated::{is_process_elevated, launch_as_admin, ElevationError};
use crate::residual::inventory::{collect_installed_apps, AppSource};
use crate::residual::now_ms;

pub const ELEVATED_ARG: &str = "--asao-bloat-apply";

pub fn history_path(dir: &Path) -> PathBuf {
    dir.join("history.json")
}
pub fn action_log(dir: &Path) -> PathBuf {
    dir.join("logs").join("actions.jsonl")
}
pub fn dry_run_log(dir: &Path) -> PathBuf {
    dir.join("logs").join("dry-run.jsonl")
}
pub fn scratch_dir(dir: &Path) -> PathBuf {
    dir.join("jobs").join("registry")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub batch_id: String,
    pub dry_run: bool,
    pub restore_point: super::executor::RestorePointResult,
    pub outcomes: Vec<super::executor::Outcome>,
    pub rejected: Vec<Rejected>,
    pub stopped: bool,
    pub admin_requested: bool,
    pub admin_declined: bool,
    pub log_path: String,
    /// Names of apps that were uninstalled, for the leftover scan hand-off.
    pub uninstalled: Vec<String>,
    /// A setting changed: some show only after File Explorer restarts.
    #[serde(default)]
    pub settings_changed: bool,
    /// A changed setting needs a restart to take effect.
    #[serde(default)]
    pub restart_needed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum JobMode {
    Apply {
        selected_ids: Vec<String>,
        #[serde(default)]
        selected_tweaks: Vec<String>,
        options: PlanOptions,
        restore_point_policy: RestorePointPolicy,
    },
    Undo {
        batch_id: String,
        action_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatJob {
    pub mode: JobMode,
    pub bloat_dir: String,
    pub batch_id: String,
    pub result_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoResult {
    pub ok: bool,
    pub message: String,
}

/// Rates every item on this PC again, without CPU sampling or usage lookups (those can only
/// make items more cautious, and ASAO already applied them before asking). Tweaks are read
/// again too, so only offered tweaks that are not in effect can be applied.
pub fn fresh_items(dir: &Path) -> Result<(Vec<BloatItem>, Win11Debloat, TweakSet), String> {
    let (db, w11d, _, _) = BloatwareDatabase::load_combined(dir);
    let inventory = collect_installed_apps();
    if inventory.failed_classes.contains(&"registry") {
        return Err("Could not read the list of installed apps.".into());
    }
    let services = collect_services().ok_or_else(|| "Could not read the list of services.".to_string())?;
    let tasks = collect_tasks().unwrap_or_default();
    let startup = collect_startup(&[]);
    let gaming = inventory.apps.iter().any(|a| {
        a.source == AppSource::Store && a.package_name.as_deref() == Some("Microsoft.GamingServices")
    });
    let device = collect_device(gaming);
    let tweaks = w11d.tweaks(device.windows_build, true);
    let p = crate::commands::apps::places();
    let mut broad = p.broad.clone();
    broad.extend(p.roots.iter().cloned());
    let samples = Samples::default();
    let last_use = |_d: &[String], _f: Option<&str>| None;
    let size = |_d: &str| None;
    let items = ScanInputs {
        db: &db,
        apps: &inventory.apps,
        details: &inventory.details,
        services: &services,
        startup: &startup,
        tasks: &tasks,
        device: &device,
        samples: &samples,
        startup_info: None,
        broad: &broad,
        windir: p.windir.as_deref(),
        last_use: &last_use,
        folder_size: &size,
        now_ms: now_ms(),
    }
    .build();
    Ok((items, w11d, tweaks))
}

/// Plans and runs a batch here. Records real batches in the history.
pub fn run_batch(
    items: &[BloatItem],
    selected_ids: &[String],
    w11d: &Win11Debloat,
    tweaks: &TweakSet,
    selected_tweaks: &[String],
    options: &PlanOptions,
    policy: RestorePointPolicy,
    dry_run: bool,
    dir: &Path,
    batch_id: &str,
    progress: &mut dyn FnMut(usize, usize, &str),
) -> ApplyReport {
    let planned = plan_with_tweaks(items, selected_ids, tweaks, selected_tweaks, options);
    let log_path = if dry_run { dry_run_log(dir) } else { action_log(dir) };
    let elevated = is_process_elevated();
    let scratch = scratch_dir(dir);
    let report: ExecReport = execute(
        &planned.actions,
        ExecOptions { dry_run, restore_point_policy: policy, elevated, w11d: Some(w11d), scratch: &scratch },
        batch_id,
        &log_path,
        progress,
    );
    if !dry_run {
        let mut history = ChangeHistory::load(&history_path(dir));
        let hint = |id: &str| items.iter().find(|i| i.id == id).and_then(|i| i.how_to_restore.clone());
        history.record(batch_id, now_ms(), &report, &hint);
        let _ = history.save(&history_path(dir));
    }
    finish(batch_id, dry_run, report, planned.rejected, &log_path)
}

fn finish(batch_id: &str, dry_run: bool, report: ExecReport, rejected: Vec<Rejected>, log_path: &Path) -> ApplyReport {
    let uninstalled = report
        .outcomes
        .iter()
        .filter(|o| o.status == OutcomeStatus::Done)
        .filter(|o| matches!(o.action.kind, ActionKind::UninstallStore { .. } | ActionKind::UninstallWin32 { .. }))
        .map(|o| o.action.item_name.clone())
        .collect();
    let settings_changed = report
        .outcomes
        .iter()
        .any(|o| o.status == OutcomeStatus::Done && matches!(o.action.kind, ActionKind::ApplyTweak { .. }));
    ApplyReport {
        settings_changed,
        restart_needed: report.restart_needed,
        batch_id: batch_id.to_string(),
        dry_run,
        restore_point: report.restore_point,
        outcomes: report.outcomes,
        rejected,
        stopped: report.stopped,
        admin_requested: false,
        admin_declined: false,
        log_path: log_path.to_string_lossy().to_string(),
        uninstalled,
        error: None,
    }
}

/// Undoes one recorded change here. The original comes from the history file.
pub fn run_undo_here(dir: &Path, batch_id: &str, action_id: &str) -> UndoResult {
    let path = history_path(dir);
    let mut history = ChangeHistory::load(&path);
    let Some(record) = history.find(batch_id, action_id) else {
        return UndoResult { ok: false, message: "That change is not in Recent changes.".into() };
    };
    if record.undone_ms.is_some() {
        return UndoResult { ok: true, message: "Already undone.".into() };
    }
    let Some(kind) = undo_for(&record.outcome) else {
        return UndoResult { ok: false, message: record.undo_hint.clone() };
    };
    match run_undo(&kind, &scratch_dir(dir)) {
        Ok(()) => {
            history.mark_undone(batch_id, action_id, now_ms());
            let _ = history.save(&path);
            UndoResult { ok: true, message: "Put back the way it was.".into() }
        }
        Err(e) => UndoResult { ok: false, message: e },
    }
}

/// Writes a job file, starts the helper as administrator and reads its result.
pub fn run_job_elevated<T: for<'de> Deserialize<'de>>(job: &mut BloatJob, dir: &Path) -> Result<T, ElevationError> {
    let jobs = dir.join("jobs");
    fs::create_dir_all(&jobs).map_err(|e| ElevationError::Failed(e.to_string()))?;
    let job_path = jobs.join(format!("job-{}.json", job.batch_id));
    let result_path = jobs.join(format!("job-{}.result.json", job.batch_id));
    job.result_path = result_path.to_string_lossy().to_string();
    fs::write(&job_path, serde_json::to_string(job).map_err(|e| ElevationError::Failed(e.to_string()))?)
        .map_err(|e| ElevationError::Failed(e.to_string()))?;
    // Win32 uninstallers wait for the user, so allow an hour.
    let launched = launch_as_admin(ELEVATED_ARG, &job_path, 60 * 60 * 1000);
    let result = fs::read_to_string(&result_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<T>(&raw).ok());
    let _ = fs::remove_file(&job_path);
    let _ = fs::remove_file(&result_path);
    launched?;
    result.ok_or_else(|| ElevationError::Failed("The administrator helper did not report back".into()))
}

/// Called first thing in `main`. Returns true when this process was the bloat helper.
pub fn maybe_run_from_args() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args[1] != ELEVATED_ARG {
        return false;
    }
    let Some(job) = fs::read_to_string(&args[2])
        .ok()
        .and_then(|raw| serde_json::from_str::<BloatJob>(&raw).ok())
    else {
        return true;
    };
    let dir = PathBuf::from(&job.bloat_dir);
    let json = match &job.mode {
        JobMode::Apply { selected_ids, selected_tweaks, options, restore_point_policy } => {
            let report = match fresh_items(&dir) {
                Ok((items, w11d, tweaks)) => run_batch(
                    &items,
                    selected_ids,
                    &w11d,
                    &tweaks,
                    selected_tweaks,
                    options,
                    *restore_point_policy,
                    false,
                    &dir,
                    &job.batch_id,
                    &mut |_, _, _| {},
                ),
                Err(e) => ApplyReport {
                    batch_id: job.batch_id.clone(),
                    dry_run: false,
                    restore_point: super::executor::RestorePointResult {
                        status: super::executor::RestorePointStatus::Skipped,
                        message: "Nothing was changed.".into(),
                    },
                    outcomes: Vec::new(),
                    rejected: Vec::new(),
                    stopped: true,
                    admin_requested: true,
                    admin_declined: false,
                    log_path: action_log(&dir).to_string_lossy().to_string(),
                    uninstalled: Vec::new(),
                    settings_changed: false,
                    restart_needed: false,
                    error: Some(e),
                },
            };
            serde_json::to_string(&report)
        }
        JobMode::Undo { batch_id, action_id } => serde_json::to_string(&run_undo_here(&dir, batch_id, action_id)),
    };
    if let Ok(json) = json {
        let _ = fs::write(&job.result_path, json);
    }
    true
}
