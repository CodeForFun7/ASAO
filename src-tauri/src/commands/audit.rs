//! Tauri commands for the Audit log page.

use serde::Serialize;
use tauri::AppHandle;

use crate::audit::{self, AuditUndo};
use crate::db::audit_log::{self, AuditEntry, AuditLog};
use crate::residual::now_ms;

fn log() -> Result<&'static AuditLog, String> {
    audit_log::global().ok_or_else(|| "The audit log is not available.".to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditPage {
    pub entries: Vec<AuditEntry>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub ok: bool,
    pub message: String,
}

/// Changes made through ASAO, newest first.
#[tauri::command]
pub fn audit_list(limit: Option<u32>, offset: Option<u32>) -> Result<AuditPage, String> {
    let log = log()?;
    Ok(AuditPage {
        entries: log.list(limit.unwrap_or(200).min(1000), offset.unwrap_or(0))?,
        total: log.count()?,
    })
}

/// Reverses one change. Runs on a worker thread because it can show an administrator prompt.
#[tauri::command]
pub async fn audit_restore(app: AppHandle, id: i64) -> Result<RestoreResult, String> {
    tauri::async_runtime::spawn_blocking(move || restore_blocking(&app, id))
        .await
        .map_err(|e| format!("Restore thread error: {}", e))?
}

fn restore_blocking(app: &AppHandle, id: i64) -> Result<RestoreResult, String> {
    let log = log()?;
    let stored = log.get(id)?.ok_or_else(|| "That entry no longer exists.".to_string())?;
    if stored.entry.restored_at.is_some() {
        return Ok(RestoreResult { ok: false, message: "This change was already restored.".into() });
    }
    let Some(raw) = stored.undo.as_deref() else {
        return Ok(RestoreResult {
            ok: false,
            message: stored.entry.undo_note.clone().unwrap_or_else(|| "This change cannot be restored.".into()),
        });
    };
    let undo: AuditUndo = serde_json::from_str(raw).map_err(|e| format!("The restore data is damaged: {}", e))?;

    match audit::run_undo(app, &undo) {
        Ok(message) => {
            log.mark_restored(id, now_ms())?;
            audit::record(
                "restore",
                &format!("Restored: {}", stored.entry.title),
                &message,
                None,
                Some("Restoring is not reversed from here. Make the change again if you need it."),
                None,
            );
            Ok(RestoreResult { ok: true, message })
        }
        Err(message) => Ok(RestoreResult { ok: false, message }),
    }
}
