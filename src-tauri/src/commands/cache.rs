//! Tauri commands for reading the SQLite scan cache.

use tauri::State;

use crate::commands::AppMonitoringState;
use crate::db::{CachedRun, RunSummary, ScanCache, ScanKind};

fn cache(state: &AppMonitoringState) -> Result<&ScanCache, String> {
    state
        .scan_cache
        .get()
        .ok_or_else(|| "The scan cache is not available.".to_string())
}

/// Newest cached run of a kind (`startup`, `storage`, `leftover`, `unused-apps`).
/// For `storage`, `scope` selects a drive such as `C:`.
#[tauri::command]
pub fn cache_get_latest(
    kind: ScanKind,
    scope: Option<String>,
    state: State<'_, AppMonitoringState>,
) -> Result<Option<CachedRun>, String> {
    cache(&state)?.latest(kind, scope.as_deref())
}

#[tauri::command]
pub fn cache_get_run(run_id: i64, state: State<'_, AppMonitoringState>) -> Result<Option<CachedRun>, String> {
    cache(&state)?.get(run_id)
}

/// Run history, newest first, without the stored results.
#[tauri::command]
pub fn cache_list_runs(
    kind: Option<ScanKind>,
    limit: Option<u32>,
    state: State<'_, AppMonitoringState>,
) -> Result<Vec<RunSummary>, String> {
    cache(&state)?.list(kind, limit.unwrap_or(50))
}

/// Deletes cached runs of one kind, or all kinds. Returns how many were removed.
#[tauri::command]
pub fn cache_clear(kind: Option<ScanKind>, state: State<'_, AppMonitoringState>) -> Result<usize, String> {
    cache(&state)?.clear(kind)
}
