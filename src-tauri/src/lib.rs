pub mod analyzer;
pub mod apps;
pub mod auth;
pub mod audit;
pub mod bloat;
pub mod bridge;
pub mod collector;
pub mod commands;
pub mod db;
pub mod residual;
pub mod settings;
pub mod startup;
pub mod tray;

use tauri::{Emitter, Manager, WindowEvent};

use commands::audit::{audit_list, audit_restore};
use commands::auth::{auth_restore_session, auth_sign_in, auth_sign_out, auth_sign_up};
use commands::cache::{cache_clear, cache_get_latest, cache_get_run, cache_list_runs};
use commands::process::{get_processes, start_monitoring, stop_monitoring};
use commands::startup::{
    disable_startup_item, enable_startup_item, get_boot_performance_summary, get_startup_items,
    wpr_cancel_boot_trace, wpr_get_status, wpr_start_boot_trace,
};
use commands::bloat::{
    bloat_apply, bloat_cancel_scan, bloat_get_info, bloat_get_last_result, bloat_history, bloat_plan,
    bloat_restart_explorer, bloat_scan, bloat_undo,
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
    get_system_temperatures,
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

            // Authenticated local tool bridge for Nyra, the Python LangGraph agent.
            bridge::start(handle.clone());

            // 0. Open the SQLite scan cache (scans still work without it)
            match handle
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())
                .and_then(|dir| db::Database::open(&dir.join("asao.db")))
            {
                Ok(database) => {
                    db::audit_log::install(db::AuditLog::new(database.clone()));
                    let _ = monitoring_state.scan_cache.set(db::ScanCache::new(database));
                }
                Err(e) => eprintln!("Scan cache and audit log disabled: {}", e),
            }

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
            auth_sign_up,
            auth_sign_in,
            auth_restore_session,
            auth_sign_out,
            get_processes,
            get_system_metrics,
            get_battery_status,
            get_system_temperatures,
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
            bloat_scan,
            bloat_cancel_scan,
            bloat_get_last_result,
            bloat_get_info,
            bloat_plan,
            bloat_apply,
            bloat_history,
            bloat_undo,
            bloat_restart_explorer,
            recycle_bin_list,
            recycle_bin_delete,
            cache_get_latest,
            cache_get_run,
            cache_list_runs,
            cache_clear,
            audit_list,
            audit_restore
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
