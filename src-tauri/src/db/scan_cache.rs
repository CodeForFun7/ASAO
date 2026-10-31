//! Cache of every scan run (Startup, Storage, Leftover, Unused apps).
//!
//! Each run is stored as one row: a few queryable columns plus the full result as JSON.
//! Only the newest `KEEP_PER_SCOPE` runs of each kind (and drive, for Storage) are kept.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::connection::Database;

const KEEP_PER_SCOPE: i64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanKind {
    Startup,
    Storage,
    Leftover,
    UnusedApps,
}

impl ScanKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ScanKind::Startup => "startup",
            ScanKind::Storage => "storage",
            ScanKind::Leftover => "leftover",
            ScanKind::UnusedApps => "unused-apps",
        }
    }
}

/// Run metadata without the (potentially large) result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub id: i64,
    pub kind: String,
    pub scope: String,
    pub started_at: i64,
    pub duration_ms: i64,
    pub item_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedRun {
    #[serde(flatten)]
    pub summary: RunSummary,
    pub payload: Value,
}

#[derive(Clone)]
pub struct ScanCache {
    db: Database,
}

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

fn summary_from_row(row: &Row<'_>) -> rusqlite::Result<RunSummary> {
    Ok(RunSummary {
        id: row.get(0)?,
        kind: row.get(1)?,
        scope: row.get(2)?,
        started_at: row.get(3)?,
        duration_ms: row.get(4)?,
        item_count: row.get(5)?,
    })
}

impl ScanCache {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Stores one run and trims older runs. `scope` is the drive for Storage, empty otherwise.
    pub fn record<T: Serialize>(
        &self,
        kind: ScanKind,
        scope: &str,
        started_at_ms: u64,
        duration_ms: u64,
        item_count: u64,
        result: &T,
    ) -> Result<i64, String> {
        let payload = serde_json::to_string(result).map_err(|e| e.to_string())?;
        let mut conn = self.db.lock()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO scan_runs (kind, scope, started_at, duration_ms, item_count, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                kind.as_str(),
                scope,
                to_i64(started_at_ms),
                to_i64(duration_ms),
                to_i64(item_count),
                payload
            ],
        )
        .map_err(|e| e.to_string())?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "DELETE FROM scan_runs
             WHERE kind = ?1 AND scope = ?2
               AND id NOT IN (
                   SELECT id FROM scan_runs WHERE kind = ?1 AND scope = ?2
                   ORDER BY id DESC LIMIT ?3
               )",
            params![kind.as_str(), scope, KEEP_PER_SCOPE],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(id)
    }

    /// Replaces the result of an existing run, for data that arrives after the scan
    /// itself finishes (background folder sizing).
    pub fn update_result<T: Serialize>(&self, run_id: i64, item_count: u64, result: &T) -> Result<(), String> {
        let payload = serde_json::to_string(result).map_err(|e| e.to_string())?;
        let conn = self.db.lock()?;
        conn.execute(
            "UPDATE scan_runs SET payload = ?1, item_count = ?2 WHERE id = ?3",
            params![payload, to_i64(item_count), run_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Newest run of a kind. `scope` narrows it (for example one drive).
    pub fn latest(&self, kind: ScanKind, scope: Option<&str>) -> Result<Option<CachedRun>, String> {
        let conn = self.db.lock()?;
        let row = conn
            .query_row(
                "SELECT id, kind, scope, started_at, duration_ms, item_count, payload
                 FROM scan_runs
                 WHERE kind = ?1 AND (?2 IS NULL OR scope = ?2)
                 ORDER BY id DESC LIMIT 1",
                params![kind.as_str(), scope],
                |r| Ok((summary_from_row(r)?, r.get::<_, String>(6)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Self::finish(row)
    }

    pub fn get(&self, run_id: i64) -> Result<Option<CachedRun>, String> {
        let conn = self.db.lock()?;
        let row = conn
            .query_row(
                "SELECT id, kind, scope, started_at, duration_ms, item_count, payload
                 FROM scan_runs WHERE id = ?1",
                params![run_id],
                |r| Ok((summary_from_row(r)?, r.get::<_, String>(6)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        Self::finish(row)
    }

    /// Newest-first run history, optionally for one kind.
    pub fn list(&self, kind: Option<ScanKind>, limit: u32) -> Result<Vec<RunSummary>, String> {
        let conn = self.db.lock()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, kind, scope, started_at, duration_ms, item_count
                 FROM scan_runs
                 WHERE (?1 IS NULL OR kind = ?1)
                 ORDER BY id DESC LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![kind.map(|k| k.as_str()), limit.min(200)], summary_from_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    }

    pub fn clear(&self, kind: Option<ScanKind>) -> Result<usize, String> {
        let conn = self.db.lock()?;
        conn.execute(
            "DELETE FROM scan_runs WHERE (?1 IS NULL OR kind = ?1)",
            params![kind.map(|k| k.as_str())],
        )
        .map_err(|e| e.to_string())
    }

    fn finish(row: Option<(RunSummary, String)>) -> Result<Option<CachedRun>, String> {
        match row {
            None => Ok(None),
            Some((summary, text)) => {
                let payload = serde_json::from_str(&text).map_err(|e| format!("Cached run is unreadable: {}", e))?;
                Ok(Some(CachedRun { summary, payload }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache() -> ScanCache {
        ScanCache::new(Database::open_in_memory().unwrap())
    }

    #[test]
    fn records_and_reads_latest() {
        let c = cache();
        c.record(ScanKind::Startup, "", 1, 5, 2, &serde_json::json!({"a": 1})).unwrap();
        let id = c.record(ScanKind::Startup, "", 2, 6, 3, &serde_json::json!({"a": 2})).unwrap();
        let latest = c.latest(ScanKind::Startup, None).unwrap().unwrap();
        assert_eq!(latest.summary.id, id);
        assert_eq!(latest.payload["a"], 2);
        assert!(c.latest(ScanKind::Leftover, None).unwrap().is_none());
    }

    #[test]
    fn keeps_only_newest_per_scope() {
        let c = cache();
        for i in 0..15 {
            c.record(ScanKind::Storage, "C:", i, 1, 0, &i).unwrap();
        }
        c.record(ScanKind::Storage, "D:", 1, 1, 0, &0).unwrap();
        assert_eq!(c.list(Some(ScanKind::Storage), 100).unwrap().len(), 11);
        assert_eq!(c.latest(ScanKind::Storage, Some("D:")).unwrap().unwrap().summary.scope, "D:");
    }

    #[test]
    fn updates_result() {
        let c = cache();
        let id = c.record(ScanKind::Leftover, "", 1, 1, 1, &serde_json::json!({"s": 0})).unwrap();
        c.update_result(id, 4, &serde_json::json!({"s": 9})).unwrap();
        let run = c.get(id).unwrap().unwrap();
        assert_eq!(run.payload["s"], 9);
        assert_eq!(run.summary.item_count, 4);
    }
}
