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
    // 2: audit log of every change made by or through ASAO, with the data needed to reverse it
    r#"
    CREATE TABLE IF NOT EXISTS audit_log (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        created_at  INTEGER NOT NULL,
        category    TEXT    NOT NULL,
        title       TEXT    NOT NULL,
        detail      TEXT    NOT NULL DEFAULT '',
        undo        TEXT,
        undo_note   TEXT,
        ref_key     TEXT,
        restored_at INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_audit_log_ref ON audit_log (ref_key);
    "#,
    // 3: some existing databases were already at version 2 (made by another build) without the
    // audit log, so migration 2 never ran there. Same statements, safe to repeat.
    r#"
    CREATE TABLE IF NOT EXISTS audit_log (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        created_at  INTEGER NOT NULL,
        category    TEXT    NOT NULL,
        title       TEXT    NOT NULL,
        detail      TEXT    NOT NULL DEFAULT '',
        undo        TEXT,
        undo_note   TEXT,
        ref_key     TEXT,
        restored_at INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_audit_log_ref ON audit_log (ref_key);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_audit_log_to_a_database_already_at_version_2() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE other (id INTEGER); PRAGMA user_version = 2;").unwrap();
        run(&mut conn).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = 'audit_log'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
