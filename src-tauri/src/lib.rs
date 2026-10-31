pub mod analyzer;
pub mod apps;
pub mod collector;
pub mod commands;
pub mod db;
pub mod residual;
pub mod settings;
pub mod startup;
pub mod tray;

use tauri::{Emitter, Manager, WindowEvent};

use commands::process::{get_processes, start_monitoring, stop_monitoring};
use commands::startup::{
    disable_startup_item, enable_startup_item, get_boot_performance_summary, get_startup_items,
    wpr_cancel_boot_trace, wpr_get_status, wpr_start_boot_trace,
};
use commands::apps::{
    apps_get_last, apps_ignore, apps_remove, apps_scan, apps_unignore,
};
use commands::recycle_bin::{recycle_bin_delete, recycle_bin_list};
use commands::residual::{
    residual_cancel_scan, residual_clean, residual_clean_empty, residual_get_ignore_list, residual_get_info,
    residual_get_last_result, residual_ignore_app, residual_ignore_folder,
    residual_open_recycle_bin, residual_scan, residual_unignore_app, residual_unignore_folder,
};
use commands::storage::{
    cancel_storage_scan, get_storage_drives, get_storage_snapshot, open_storage_location,
    read_storage_directory, search_storage_items, start_storage_scan,
};
use commands::system::{
    get_battery_status, get_categorized_recommendations, get_full_snapshot, get_settings, get_system_metrics,
    get_widget_update, get_widget_visibility, hide_widget, open_main_window,
    show_widget, toggle_widget, update_settings, window_close, window_minimize,
    window_toggle_maximize,
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

            // Local bridge used only by the Python agent's get_info tool.
            let bridge_state = monitoring_state.clone();
            std::thread::spawn(move || {
                let Ok(server) = tiny_http::Server::http("127.0.0.1:8766") else {
                    eprintln!("Nyra Rust tool bridge could not bind 127.0.0.1:8766");
                    return;
                };
                for request in server.incoming_requests() {
                    let allowed = request.method() == &tiny_http::Method::Get
                        && request.url() == "/get_info";
                    if !allowed {
                        let _ = request.respond(tiny_http::Response::empty(404));
                        continue;
                    }
                    let result = bridge_state.engine.lock()
                        .map_err(|_| "Failed to acquire system analyzer lock".to_string())
                        .and_then(|mut engine| engine.get_latest_or_collect())
                        .map(|snapshot| serde_json::json!({ "metrics": snapshot.metrics, "processes": snapshot.processes }));
                    let (status, body) = match result {
                        Ok(data) => (tiny_http::StatusCode(200), data.to_string()),
                        Err(error) => (tiny_http::StatusCode(500), error),
                    };
                    let response = tiny_http::Response::from_string(body)
                        .with_status_code(status)
                        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap());
                    let _ = request.respond(response);
                }
            });

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
            monitoring_state.ensure_worker_loop(handle.clone());

            // 5. Leftover cleanup: record installed apps quietly in the background, then run
            //    one scan so the Storage suggestion has numbers. Nothing is deleted from here.
            commands::residual::start_background_work(handle.clone(), monitoring_state.clone());

            // 6. Unused apps: remember which programs run, so "last used" improves over time.
            commands::apps::start_usage_tracker(handle, monitoring_state.clone());

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
            get_battery_status,
            get_full_snapshot,
            get_categorized_recommendations,
            get_widget_update,
            get_widget_visibility,
            get_settings,
            update_settings,
            show_widget,
            hide_widget,
            toggle_widget,
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
            open_storage_location,
            residual_scan,
            residual_cancel_scan,
            residual_get_last_result,
            residual_clean,
            residual_clean_empty,
            residual_get_ignore_list,
            residual_ignore_folder,
            residual_ignore_app,
            residual_unignore_folder,
            residual_unignore_app,
            residual_open_recycle_bin,
            residual_get_info,
            apps_scan,
            apps_get_last,
            apps_remove,
            apps_ignore,
            apps_unignore,
            recycle_bin_list,
            recycle_bin_delete
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
