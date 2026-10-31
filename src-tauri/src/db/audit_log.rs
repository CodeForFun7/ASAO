//! Audit log: one row per change made by or through ASAO.
//!
//! `undo` holds the JSON that `crate::audit::AuditUndo` needs to reverse the change, or NULL
//! when the change cannot be reversed (then `undo_note` says why). Rows are listed newest first.

use std::sync::OnceLock;

use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use super::connection::Database;

static GLOBAL: OnceLock<AuditLog> = OnceLock::new();

/// The process-wide audit log. Empty if the database could not open at startup.
pub fn global() -> Option<&'static AuditLog> {
    GLOBAL.get()
}

pub fn install(log: AuditLog) {
    let _ = GLOBAL.set(log);
}

pub struct NewEntry {
    pub created_at: u64,
    pub category: String,
    pub title: String,
    pub detail: String,
    pub undo: Option<String>,
    pub undo_note: Option<String>,
    pub ref_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub id: i64,
    pub created_at: i64,
    pub category: String,
    pub title: String,
    pub detail: String,
    /// A Restore button works for this entry (it can be reversed and has not been yet).
    pub restorable: bool,
    /// Why it cannot be restored.
    pub undo_note: Option<String>,
    pub restored_at: Option<i64>,
}

/// A stored row including the undo payload (never sent to the UI).
pub struct StoredEntry {
    pub entry: AuditEntry,
    pub undo: Option<String>,
}

#[derive(Clone)]
pub struct AuditLog {
    db: Database,
}

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<StoredEntry> {
    let undo: Option<String> = row.get(5)?;
    let restored_at: Option<i64> = row.get(8)?;
    Ok(StoredEntry {
        entry: AuditEntry {
            id: row.get(0)?,
            created_at: row.get(1)?,
            category: row.get(2)?,
            title: row.get(3)?,
            detail: row.get(4)?,
            restorable: undo.is_some() && restored_at.is_none(),
            undo_note: row.get(6)?,
            restored_at,
        },
        undo,
    })
}

const COLUMNS: &str = "id, created_at, category, title, detail, undo, undo_note, ref_key, restored_at";

impl AuditLog {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn record(&self, e: NewEntry) -> Result<i64, String> {
        let conn = self.db.lock()?;
        conn.execute(
            "INSERT INTO audit_log (created_at, category, title, detail, undo, undo_note, ref_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![to_i64(e.created_at), e.category, e.title, e.detail, e.undo, e.undo_note, e.ref_key],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    /// Newest first.
    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<AuditEntry>, String> {
        let conn = self.db.lock()?;
        let mut stmt = conn
            .prepare(&format!("SELECT {COLUMNS} FROM audit_log ORDER BY id DESC LIMIT ?1 OFFSET ?2"))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit, offset], from_row)
            .map_err(|e| e.to_string())?;
        rows.map(|r| r.map(|s| s.entry).map_err(|e| e.to_string())).collect()
    }

    pub fn count(&self) -> Result<i64, String> {
        let conn = self.db.lock()?;
        conn.query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))
            .map_err(|e| e.to_string())
    }

    pub fn get(&self, id: i64) -> Result<Option<StoredEntry>, String> {
        let conn = self.db.lock()?;
        conn.query_row(&format!("SELECT {COLUMNS} FROM audit_log WHERE id = ?1"), params![id], from_row)
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn mark_restored(&self, id: i64, when: u64) -> Result<(), String> {
        let conn = self.db.lock()?;
        conn.execute(
            "UPDATE audit_log SET restored_at = ?2 WHERE id = ?1 AND restored_at IS NULL",
            params![id, to_i64(when)],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Marks entries restored from outside the Audit log page (for example Debloat's Undo).
    /// Returns how many reversible entries were marked.
    pub fn mark_restored_by_ref(&self, ref_key: &str, when: u64) -> Result<usize, String> {
        let conn = self.db.lock()?;
        conn.execute(
            "UPDATE audit_log SET restored_at = ?2
             WHERE ref_key = ?1 AND restored_at IS NULL AND undo IS NOT NULL",
            params![ref_key, to_i64(when)],
        )
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, undo: Option<&str>) -> NewEntry {
        NewEntry {
            created_at: 1,
            category: "test".into(),
            title: title.into(),
            detail: String::new(),
            undo: undo.map(String::from),
            undo_note: None,
            ref_key: Some(format!("ref:{title}")),
        }
    }

    #[test]
    fn lists_newest_first_and_tracks_restore() {
        let log = AuditLog::new(Database::open_in_memory().unwrap());
        let a = log.record(entry("a", Some("{}"))).unwrap();
        let b = log.record(entry("b", None)).unwrap();
        let list = log.list(10, 0).unwrap();
        assert_eq!(list.iter().map(|e| e.id).collect::<Vec<_>>(), vec![b, a]);
        assert!(list[1].restorable && !list[0].restorable);

        log.mark_restored_by_ref("ref:a", 5).unwrap();
        let a_row = log.get(a).unwrap().unwrap();
        assert_eq!(log.mark_restored_by_ref("ref:b", 5).unwrap(), 0);
        assert_eq!(a_row.entry.restored_at, Some(5));
        assert!(!a_row.entry.restorable);
    }
}
