use tauri::State;

use crate::analyzer::{ProcessSnapshotPayload, SystemMetrics};
use crate::commands::AppMonitoringState;

#[tauri::command]
pub fn get_system_metrics(state: State<'_, AppMonitoringState>) -> Result<SystemMetrics, String> {
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| "Failed to acquire system analyzer lock".to_string())?;
    let snapshot = engine.get_latest_or_collect()?;
    Ok(snapshot.metrics)
}

#[tauri::command]
pub fn get_full_snapshot(
    state: State<'_, AppMonitoringState>,
) -> Result<ProcessSnapshotPayload, String> {
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| "Failed to acquire system analyzer lock".to_string())?;
    engine.get_latest_or_collect()
}
