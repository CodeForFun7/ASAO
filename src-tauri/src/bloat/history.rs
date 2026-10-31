//! `ChangeHistory`: every real batch of changes with the original states, for Recent changes
//! and undo. Dry runs are only logged, not kept here.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::executor::{ExecReport, Original, Outcome, OutcomeStatus, RestorePointResult};
use super::planner::ActionKind;

const MAX_BATCHES: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAction {
    pub outcome: Outcome,
    pub undone_ms: Option<u64>,
    /// How this change can be undone, in plain words.
    pub undo_hint: String,
    /// ASAO can undo it with one click (services, startup entries, tasks).
    pub can_undo: bool,
    pub store_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub id: String,
    pub time_ms: u64,
    pub restore_point: RestorePointResult,
    pub actions: Vec<HistoryAction>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeHistory {
    pub batches: Vec<Batch>,
}

/// The undo for one recorded change: what to run, if ASAO can do it itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum UndoKind {
    RestoreStartup { key: String, values: Vec<(String, Option<String>)> },
    EnableTask { path: String },
    RestoreService { name: String, start_type: String, delayed: bool, running: bool },
    RestoreRegistry { backups: Vec<super::regfile::RegBackup> },
}

impl UndoKind {
    pub fn needs_admin(&self) -> bool {
        match self {
            UndoKind::RestoreStartup { key, .. } => key.starts_with("HKLM"),
            UndoKind::EnableTask { .. } | UndoKind::RestoreService { .. } => true,
            UndoKind::RestoreRegistry { backups } => backups.iter().any(|b| {
                let key = match b {
                    super::regfile::RegBackup::Value { key, .. } | super::regfile::RegBackup::Key { key, .. } => key,
                };
                !key.starts_with("HKEY_CURRENT_USER")
            }),
        }
    }
}

/// The automatic undo for a change that went through, if there is one.
pub fn undo_for(o: &Outcome) -> Option<UndoKind> {
    if o.status != OutcomeStatus::Done {
        return None;
    }
    match (&o.action.kind, &o.original) {
        (ActionKind::DisableStartup { .. }, Some(Original::StartupApproved { key, values })) => {
            Some(UndoKind::RestoreStartup { key: key.clone(), values: values.clone() })
        }
        (ActionKind::DisableTask { path }, Some(Original::Task { enabled: true })) => {
            Some(UndoKind::EnableTask { path: path.clone() })
        }
        (ActionKind::ApplyTweak { .. }, Some(Original::Registry { backups })) => {
            Some(UndoKind::RestoreRegistry { backups: backups.clone() })
        }
        (ActionKind::ConfigureService { name, .. }, Some(Original::Service { start_type, delayed, running })) => {
            Some(UndoKind::RestoreService {
                name: name.clone(),
                start_type: start_type.clone(),
                delayed: *delayed,
                running: *running,
            })
        }
        _ => None,
    }
}

fn describe(o: &Outcome, how_to_restore: Option<&str>) -> (String, Option<String>) {
    match &o.action.kind {
        ActionKind::UninstallStore { package_full_name, .. } => {
            let family = store_family(package_full_name);
            (
                "Reinstall it from the Microsoft Store. Undo opens its Store page.".into(),
                family.map(|f| format!("ms-windows-store://pdp/?PFN={}", f)),
            )
        }
        ActionKind::RemoveProvisioned { .. } => (
            "New users will not get it. Anyone can still install it from the Microsoft Store.".into(),
            None,
        ),
        ActionKind::UninstallWin32 { .. } => (
            format!(
                "ASAO can't reinstall it automatically. {} You can also go back to the restore point made before this change.",
                how_to_restore.unwrap_or("Download it again from the publisher.")
            ),
            None,
        ),
        ActionKind::ApplyTweak { .. } => (
            "Undo puts back the exact registry values it replaced. Some settings show after you sign out or restart File Explorer.".into(),
            None,
        ),
        _ => ("Undo puts it back the way it was.".into(), None),
    }
}

/// `Name_1.2.3.4_x64__publisherid` → `Name_publisherid`.
pub fn store_family(full_name: &str) -> Option<String> {
    let mut parts = full_name.split('_');
    let name = parts.next()?;
    let publisher = full_name.rsplit('_').next()?;
    if publisher.is_empty() || name.is_empty() || name == publisher {
        return None;
    }
    Some(format!("{}_{}", name, publisher))
}

impl ChangeHistory {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    /// Records a real batch. `restore_hint` gives the database's reinstall text per item id.
    pub fn record(&mut self, id: &str, time_ms: u64, report: &ExecReport, restore_hint: &dyn Fn(&str) -> Option<String>) {
        let actions = report
            .outcomes
            .iter()
            .filter(|o| matches!(o.status, OutcomeStatus::Done | OutcomeStatus::NotConfirmed))
            .map(|o| {
                let hint = restore_hint(&o.action.item_id);
                let (undo_hint, store_url) = describe(o, hint.as_deref());
                HistoryAction {
                    can_undo: undo_for(o).is_some(),
                    outcome: o.clone(),
                    undone_ms: None,
                    undo_hint,
                    store_url,
                }
            })
            .collect::<Vec<_>>();
        if actions.is_empty() {
            return;
        }
        self.batches.insert(
            0,
            Batch { id: id.to_string(), time_ms, restore_point: report.restore_point.clone(), actions },
        );
        self.batches.truncate(MAX_BATCHES);
    }

    pub fn find(&self, batch_id: &str, action_id: &str) -> Option<&HistoryAction> {
        self.batches
            .iter()
            .find(|b| b.id == batch_id)?
            .actions
            .iter()
            .find(|a| a.outcome.action.id == action_id)
    }

    pub fn mark_undone(&mut self, batch_id: &str, action_id: &str, when: u64) {
        if let Some(b) = self.batches.iter_mut().find(|b| b.id == batch_id) {
            if let Some(a) = b.actions.iter_mut().find(|a| a.outcome.action.id == action_id) {
                a.undone_ms = Some(when);
            }
        }
    }
}

/// Runs an undo. Read the original back from history, never from the caller.
pub fn run_undo(kind: &UndoKind, scratch: &std::path::Path) -> Result<(), String> {
    use super::executor::{restore_approved, restore_service, set_task};
    match kind {
        UndoKind::RestoreRegistry { backups } => super::regfile::restore(backups, scratch),
        UndoKind::RestoreStartup { key, values } => restore_approved(key, values),
        UndoKind::EnableTask { path } => set_task(path, true),
        UndoKind::RestoreService { name, start_type, delayed, running } => {
            restore_service(name, start_type, *delayed, *running)
        }
    }
}
