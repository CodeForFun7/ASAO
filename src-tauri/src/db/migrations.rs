use rusqlite::Connection;

/// Ordered list of migrations. Never edit a shipped entry; append a new one.
/// The database `user_version` is the number of migrations already applied.
const MIGRATIONS: &[&str] = &[
    // 1: cache of every scan run
    r#"
    CREATE TABLE scan_runs (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        kind         TEXT    NOT NULL,
        scope        TEXT    NOT NULL DEFAULT '',
        started_at   INTEGER NOT NULL,
        duration_ms  INTEGER NOT NULL,
        item_count   INTEGER NOT NULL DEFAULT 0,
        payload      TEXT    NOT NULL
    );
    CREATE INDEX idx_scan_runs_kind_scope_id ON scan_runs (kind, scope, id DESC);
    "#,
];

pub fn run(conn: &mut Connection) -> Result<(), String> {
    let current: usize = conn
        .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())? as usize;

    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(sql)
            .map_err(|e| format!("Migration {} failed: {}", index + 1, e))?;
        tx.execute_batch(&format!("PRAGMA user_version = {};", index + 1))
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(())
}
