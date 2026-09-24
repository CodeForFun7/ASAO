use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};

use crate::analyzer::ProcessInfo;
use crate::commands::{
    AppMonitoringState, EVENT_PROCESS_ERROR, EVENT_PROCESS_SNAPSHOT, EVENT_SYSTEM_METRICS,
};

#[tauri::command]
pub fn get_processes(state: State<'_, AppMonitoringState>) -> Result<Vec<ProcessInfo>, String> {
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| "Failed to acquire process analyzer lock".to_string())?;
    let snapshot = engine.get_latest_or_collect()?;
    Ok(snapshot.processes)
}

#[tauri::command]
pub fn start_monitoring(
    app_handle: AppHandle,
    state: State<'_, AppMonitoringState>,
) -> Result<(), String> {
    state.is_monitoring.store(true, Ordering::SeqCst);
    state.ensure_worker_loop(app_handle.clone());

    // Emit an immediate snapshot so the UI populates without delay
    if let Ok(mut engine) = state.engine.lock() {
        match engine.get_latest_or_collect() {
            Ok(snapshot) => {
                let _ = app_handle.emit(EVENT_PROCESS_SNAPSHOT, &snapshot.processes);
                let _ = app_handle.emit(EVENT_SYSTEM_METRICS, &snapshot.metrics);
            }
            Err(err) => {
                let _ = app_handle.emit(EVENT_PROCESS_ERROR, err);
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn stop_monitoring(state: State<'_, AppMonitoringState>) -> Result<(), String> {
    state.is_monitoring.store(false, Ordering::SeqCst);
    Ok(())
}
