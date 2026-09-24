use crate::analyzer::{ProcessInfo, SystemMetrics};

/// SQLite-ready persistence schema & repository contract.
///
/// In the current live-analyzer phase, metrics are maintained in bounded memory
/// and streamed over Tauri events without writing to SQLite every second.
/// When historical persistence is activated, `SqliteStorageRepository` can implement
/// `TelemetryRepository` using the DDL below.
pub const SQLITE_SCHEMA_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS applications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    executable_name TEXT NOT NULL,
    executable_path TEXT UNIQUE,
    publisher TEXT,
    product_name TEXT,
    category TEXT NOT NULL,
    is_system_critical INTEGER NOT NULL DEFAULT 0,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS process_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    application_id INTEGER NOT NULL,
    pid INTEGER NOT NULL,
    cpu_percent REAL NOT NULL,
    memory_bytes INTEGER NOT NULL,
    disk_bytes_per_sec INTEGER NOT NULL,
    network_bytes_per_sec INTEGER NOT NULL,
    status TEXT NOT NULL,
    sampled_at INTEGER NOT NULL,
    FOREIGN KEY(application_id) REFERENCES applications(id)
);

CREATE TABLE IF NOT EXISTS application_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    application_id INTEGER NOT NULL,
    date_bucket TEXT NOT NULL,
    avg_cpu_percent REAL NOT NULL,
    peak_memory_bytes INTEGER NOT NULL,
    active_seconds INTEGER NOT NULL,
    FOREIGN KEY(application_id) REFERENCES applications(id)
);

CREATE TABLE IF NOT EXISTS recommendations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    application_id INTEGER NOT NULL,
    summary TEXT NOT NULL,
    reason TEXT NOT NULL,
    recommendation TEXT,
    risk_level TEXT,
    confidence REAL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(application_id) REFERENCES applications(id)
);

CREATE TABLE IF NOT EXISTS optimization_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recommendation_id INTEGER,
    action_type TEXT NOT NULL,
    backup_payload TEXT NOT NULL,
    applied_at INTEGER NOT NULL,
    rolled_back_at INTEGER
);
"#;

pub trait TelemetryRepository: Send + Sync {
    fn record_batch_summary(&mut self, metrics: &SystemMetrics, top_processes: &[ProcessInfo]);
}

#[derive(Default)]
pub struct InMemoryTelemetryBuffer {
    pub recent_system_samples: usize,
}

impl TelemetryRepository for InMemoryTelemetryBuffer {
    fn record_batch_summary(&mut self, _metrics: &SystemMetrics, _top_processes: &[ProcessInfo]) {
        self.recent_system_samples = self.recent_system_samples.saturating_add(1);
    }
}
