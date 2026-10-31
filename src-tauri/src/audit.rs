//! Audit trail: records every change made by or through ASAO in the SQLite `audit_log` table
//! and knows how to reverse the ones that can be reversed.
//!
//! Recording is best effort. A problem with the database is logged and never fails the change.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::db::audit_log::{self, NewEntry};
use crate::residual::now_ms;

/// Which list an ignore entry belongs to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "list", rename_all = "kebab-case")]
pub enum IgnoreOp {
    Folder { path: String },
    App { key: String, name: String },
    UnusedApp { key: String, name: String },
}

/// What it takes to reverse one recorded change. Stored as JSON in `audit_log.undo`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum AuditUndo {
    /// Switch a startup entry on or off again. `service_start` is the exact start type a
    /// service had before the change.
    Startup {
        enable: bool,
        item_id: String,
        source: String,
        name: Option<String>,
        registry_key: Option<String>,
        service_name: Option<String>,
        task_path: Option<String>,
        command_line: Option<String>,
        service_start: Option<String>,
    },
    /// A Debloat change; its original state lives in the Debloat change history.
    Bloat { batch_id: String, action_id: String },
    /// Bring a file or folder back from the Recycle Bin to its original place.
    RestoreRecycled { path: String },
    /// Add (`add`) or remove an entry of the ignore list.
    Ignore { op: IgnoreOp, add: bool },
}

/// Records one change. `undo` is None when the change cannot be reversed; `note` then says why.
pub fn record(
    category: &str,
    title: &str,
    detail: &str,
    undo: Option<AuditUndo>,
    note: Option<&str>,
    ref_key: Option<String>,
) {
    let Some(log) = audit_log::global() else { return };
    let undo_json = match undo.map(|u| serde_json::to_string(&u)) {
        Some(Ok(s)) => Some(s),
        Some(Err(e)) => {
            eprintln!("Could not serialise audit undo: {}", e);
            None
        }
        None => None,
    };
    let undo_note = if undo_json.is_none() { note.map(String::from) } else { None };
    if let Err(e) = log.record(NewEntry {
        created_at: now_ms(),
        category: category.to_string(),
        title: title.to_string(),
        detail: detail.to_string(),
        undo: undo_json,
        undo_note,
        ref_key,
    }) {
        eprintln!("Could not write audit log: {}", e);
    }
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

/// Audit entry for a file or folder that went to the Recycle Bin or was deleted for good.
pub fn record_deletion(category: &str, label: &str, path: &str, size_bytes: Option<u64>, recycled: bool) {
    let size = size_bytes.filter(|s| *s > 0).map(|s| format!(" ({})", human_bytes(s))).unwrap_or_default();
    if recycled {
        record(
            category,
            &format!("Moved {} to the Recycle Bin", label),
            &format!("{}{}", path, size),
            Some(AuditUndo::RestoreRecycled { path: path.to_string() }),
            None,
            None,
        );
    } else {
        record(
            category,
            &format!("Permanently deleted {}", label),
            &format!("{}{}", path, size),
            None,
            Some("Deleted permanently. It is not in the Recycle Bin, so ASAO cannot bring it back."),
            None,
        );
    }
}

fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

fn restore_recycled(path: &str) -> Result<String, String> {
    let script = format!(
        r#"
$target = ('{}').TrimEnd('\')
$shell = New-Object -ComObject Shell.Application
$bin = $shell.Namespace(10)
$found = $null
foreach ($i in $bin.Items()) {{
    $from = $i.ExtendedProperty('System.Recycle.DeletedFrom')
    if (-not $from) {{ continue }}
    if ((Join-Path $from $i.Name).TrimEnd('\') -ieq $target) {{ $found = $i; break }}
}}
if (-not $found) {{ throw 'It is no longer in the Recycle Bin.' }}
$found.InvokeVerb('undelete')
Start-Sleep -Milliseconds 800
if (-not (Test-Path -LiteralPath $target)) {{ throw 'Windows did not put it back. The original folder may be missing.' }}
"#,
        ps_quote(path)
    );
    crate::bloat::executor::ps(&script)?;
    Ok(format!("Restored {}", path))
}

fn run_ignore(app: &AppHandle, op: &IgnoreOp, add: bool) -> Result<String, String> {
    let files = crate::commands::residual::residual_files(app)?;
    let mut list = crate::residual::store_files::IgnoreList::load(&files.ignore_list());
    let now = now_ms();
    match (op, add) {
        (IgnoreOp::Folder { path }, true) => list.add_folder(path, now),
        (IgnoreOp::Folder { path }, false) => list.remove_folder(path),
        (IgnoreOp::App { key, name }, true) => list.add_app(key, name, now),
        (IgnoreOp::App { key, .. }, false) => list.remove_app(key),
        (IgnoreOp::UnusedApp { key, name }, true) => list.add_unused_app(key, name, now),
        (IgnoreOp::UnusedApp { key, .. }, false) => list.remove_unused_app(key),
    }
    list.save(&files.ignore_list())?;
    Ok(if add { "Added back to the ignore list." } else { "Removed from the ignore list." }.to_string())
}

/// Reverses one recorded change. Does not write to the audit log itself.
pub fn run_undo(app: &AppHandle, undo: &AuditUndo) -> Result<String, String> {
    use crate::commands::startup as su;
    match undo {
        AuditUndo::Startup {
            enable,
            item_id,
            source,
            name,
            registry_key,
            service_name,
            task_path,
            command_line,
            service_start,
        } => {
            if let (Some(svc), Some(start)) = (service_name.as_deref(), service_start.as_deref()) {
                su::restore_service_start(svc, start)?;
            } else if *enable {
                su::do_enable_startup_item(
                    item_id.clone(),
                    source.clone(),
                    name.clone(),
                    registry_key.clone(),
                    service_name.clone(),
                    task_path.clone(),
                    command_line.clone(),
                )?;
            } else {
                su::do_disable_startup_item(
                    item_id.clone(),
                    source.clone(),
                    name.clone(),
                    registry_key.clone(),
                    service_name.clone(),
                    task_path.clone(),
                )?;
            }
            Ok("The startup entry is back the way it was.".into())
        }
        AuditUndo::Bloat { batch_id, action_id } => {
            let r = crate::commands::bloat::undo_blocking(app, batch_id.clone(), action_id.clone())?;
            if r.ok {
                Ok(r.message)
            } else {
                Err(r.message)
            }
        }
        AuditUndo::RestoreRecycled { path } => restore_recycled(path),
        AuditUndo::Ignore { op, add } => run_ignore(app, op, *add),
    }
}
