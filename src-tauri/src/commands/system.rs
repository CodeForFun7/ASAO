use tauri::{State, WebviewWindow};

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

#[tauri::command]
pub fn window_minimize(window: WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn window_toggle_maximize(window: WebviewWindow) -> Result<bool, String> {
    let is_max = window.is_maximized().map_err(|e| e.to_string())?;
    if is_max {
        window.unmaximize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[tauri::command]
pub fn window_close(window: WebviewWindow) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}
