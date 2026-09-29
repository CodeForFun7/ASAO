pub mod analyzer;
pub mod collector;
pub mod commands;
pub mod db;
pub mod settings;
pub mod startup;
pub mod tray;

use tauri::{Emitter, Manager, WindowEvent};

use commands::process::{get_processes, start_monitoring, stop_monitoring};
use commands::startup::{
    disable_startup_item, enable_startup_item, get_boot_performance_summary, get_startup_items,
    wpr_cancel_boot_trace, wpr_get_status, wpr_start_boot_trace,
};
use commands::storage::{
    cancel_storage_scan, get_storage_drives, get_storage_snapshot, open_storage_location,
    read_storage_directory, search_storage_items, start_storage_scan,
};
use commands::system::{
    get_full_snapshot, get_settings, get_system_metrics, get_widget_update, get_widget_visibility,
    hide_widget, open_main_window, set_widget_mode, show_widget, toggle_widget, update_settings,
    window_close, window_minimize, window_toggle_maximize,
};
use commands::AppMonitoringState;
use settings::{
    apply_widget_window_geometry, enforce_widget_topmost, load_settings, save_settings,
};
use tray::setup_system_tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let monitoring_state = AppMonitoringState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(monitoring_state.clone())
        .setup(move |app| {
            let handle = app.handle().clone();

            // 1. Load persisted settings
            let loaded_settings = load_settings(&handle);
            if let Ok(mut guard) = monitoring_state.settings.lock() {
                *guard = loaded_settings.clone();
            }

            // 2. Initialize System Tray
            if let Ok(tray_handles) = setup_system_tray(&handle, loaded_settings.widget_enabled) {
                if let Ok(mut t_guard) = monitoring_state.tray_handles.lock() {
                    *t_guard = Some(tray_handles);
                }
            }

            // 3. Configure dedicated Widget Window geometry & startup visibility rule
            if let Some(widget_win) = app.get_webview_window("widget") {
                apply_widget_window_geometry(&widget_win, &loaded_settings);
                if loaded_settings.widget_enabled && loaded_settings.launch_widget_on_startup {
                    let _ = widget_win.show();
                    enforce_widget_topmost(&widget_win, loaded_settings.always_on_top);
                    let _ = handle.emit("widget:visibility", true);
                } else {
                    let _ = widget_win.hide();
                    let _ = handle.emit("widget:visibility", false);
                }
            }

            // 4. Start centralized 1Hz telemetry worker loop
            monitoring_state.ensure_worker_loop(handle);

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "widget" {
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        // Closing the widget hides it rather than terminating Asao
                        api.prevent_close();
                        let _ = window.hide();
                        let _ = window.app_handle().emit("widget:visibility", false);
                    }
                    WindowEvent::Focused(_) => {
                        let app_handle = window.app_handle();
                        if let Some(state) = app_handle.try_state::<AppMonitoringState>() {
                            let always_top = state
                                .settings
                                .lock()
                                .map(|g| g.always_on_top)
                                .unwrap_or(true);
                            if let Some(widget_win) = app_handle.get_webview_window("widget") {
                                enforce_widget_topmost(&widget_win, always_top);
                            }
                        }
                    }
                    WindowEvent::Moved(pos) => {
                        // Persist manual dragging position safely inside screen bounds
                        if pos.x > -10000 && pos.y > -10000 {
                            let app_handle = window.app_handle();
                            if let Some(state) = app_handle.try_state::<AppMonitoringState>() {
                                if let Ok(mut guard) = state.settings.lock() {
                                    guard.widget_position = "custom".to_string();
                                    guard.custom_x = Some(pos.x);
                                    guard.custom_y = Some(pos.y);
                                    let _ = save_settings(app_handle, &guard);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_processes,
            get_system_metrics,
            get_full_snapshot,
            get_widget_update,
            get_widget_visibility,
            get_settings,
            update_settings,
            show_widget,
            hide_widget,
            toggle_widget,
            set_widget_mode,
            open_main_window,
            start_monitoring,
            stop_monitoring,
            window_minimize,
            window_toggle_maximize,
            window_close,
            get_startup_items,
            disable_startup_item,
            enable_startup_item,
            get_boot_performance_summary,
            wpr_get_status,
            wpr_start_boot_trace,
            wpr_cancel_boot_trace,
            get_storage_drives,
            get_storage_snapshot,
            start_storage_scan,
            cancel_storage_scan,
            read_storage_directory,
            search_storage_items,
            open_storage_location
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
