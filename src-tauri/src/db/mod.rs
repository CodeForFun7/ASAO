//! SQLite persistence.
//!
//! - `connection`: opens the database file and owns the shared connection.
//! - `migrations`: versioned schema (tracked with `PRAGMA user_version`).
//! - `scan_cache`: stores the result of every Startup, Storage, Leftover and Unused apps run.
//! - `telemetry`: reserved contract for live process telemetry (not persisted today).

pub mod connection;
pub mod migrations;
pub mod scan_cache;
pub mod telemetry;

pub use connection::Database;
pub use scan_cache::{CachedRun, RunSummary, ScanCache, ScanKind};
pub use telemetry::{InMemoryTelemetryBuffer, TelemetryRepository, SQLITE_SCHEMA_DDL};
