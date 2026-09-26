use serde::Serialize;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, Size, State, WebviewWindow};

use crate::analyzer::process_analyzer::ProcessAnalyzerEngine;
use crate::analyzer::{ProcessSnapshotPayload, SystemMetrics, WidgetSystemUpdate};
use crate::commands::AppMonitoringState;
use crate::settings::{apply_widget_window_geometry, save_settings, AsaoSettings};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateInspectPayload {
    pub route: String,
    pub pid: Option<u32>,
}

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
pub fn get_widget_update(
    state: State<'_, AppMonitoringState>,
) -> Result<WidgetSystemUpdate, String> {
    let mut engine = state
        .engine
        .lock()
        .map_err(|_| "Failed to acquire system analyzer lock".to_string())?;
    let snapshot = engine.get_latest_or_collect()?;
    Ok(ProcessAnalyzerEngine::build_widget_update(&snapshot))
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppMonitoringState>) -> Result<AsaoSettings, String> {
    let guard = state
        .settings
        .lock()
        .map_err(|_| "Failed to read settings lock".to_string())?;
    Ok(guard.clone())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
    mut new_settings: AsaoSettings,
) -> Result<AsaoSettings, String> {
    new_settings.widget_opacity = new_settings.widget_opacity.clamp(60, 100);

    {
        let mut guard = state
            .settings
            .lock()
            .map_err(|_| "Failed to acquire settings lock".to_string())?;
        *guard = new_settings.clone();
    }

    let _ = save_settings(&app, &new_settings);

    // Sync system tray menu enabled/disabled state with widget_enabled
    if let Ok(tray_guard) = state.tray_handles.lock() {
        if let Some(ref handles) = *tray_guard {
            let _ = handles
                .show_widget_item
                .set_enabled(new_settings.widget_enabled);
            let _ = handles
                .hide_widget_item
                .set_enabled(new_settings.widget_enabled);
        }
    }

    // Apply widget window settings
    if let Some(widget_win) = app.get_webview_window("widget") {
        let _ = widget_win.set_always_on_top(new_settings.always_on_top);
        if !new_settings.widget_enabled {
            let _ = widget_win.hide();
        } else {
            apply_widget_window_geometry(&widget_win, &new_settings);
        }
    }

    let _ = app.emit("settings:updated", &new_settings);
    Ok(new_settings)
}

#[tauri::command]
pub fn show_widget(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
) -> Result<bool, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "Failed to read settings".to_string())?
        .clone();

    if !settings.widget_enabled {
        return Ok(false);
    }

    if let Some(widget_win) = app.get_webview_window("widget") {
        apply_widget_window_geometry(&widget_win, &settings);
        let _ = widget_win.show();
        let _ = widget_win.set_focus();
        return Ok(true);
    }

    Ok(false)
}

#[tauri::command]
pub fn hide_widget(app: AppHandle) -> Result<(), String> {
    if let Some(widget_win) = app.get_webview_window("widget") {
        let _ = widget_win.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn set_widget_mode(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
    mode: String,
) -> Result<(), String> {
    if let Some(widget_win) = app.get_webview_window("widget") {
        let (width, height) = if mode == "chat" {
            (352.0, 520.0)
        } else {
            (328.0, 412.0)
        };
        let _ = widget_win.set_size(Size::Logical(LogicalSize::new(width, height)));

        if let Ok(guard) = state.settings.lock() {
            apply_widget_window_geometry(&widget_win, &guard);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn open_main_window(
    app: AppHandle,
    route: Option<String>,
    inspect_pid: Option<u32>,
) -> Result<(), String> {
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.unminimize();
        let _ = main_win.show();
        let _ = main_win.set_focus();
    }

    let payload = NavigateInspectPayload {
        route: route.unwrap_or_else(|| "dashboard".to_string()),
        pid: inspect_pid,
    };
    let _ = app.emit("asao:navigate-inspect", &payload);
    Ok(())
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
    if window.label() == "widget" {
        window.hide().map_err(|e| e.to_string())
    } else {
        window.close().map_err(|e| e.to_string())
    }
}
