pub mod analyzer;
pub mod collector;
pub mod commands;
pub mod db;

use commands::process::{get_processes, start_monitoring, stop_monitoring};
use commands::system::{get_full_snapshot, get_system_metrics};
use commands::AppMonitoringState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let monitoring_state = AppMonitoringState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(monitoring_state.clone())
        .setup(move |app| {
            monitoring_state.ensure_worker_loop(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_processes,
            get_system_metrics,
            get_full_snapshot,
            start_monitoring,
            stop_monitoring
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
