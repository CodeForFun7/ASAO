use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsaoSettings {
    pub start_with_windows: bool,
    pub widget_enabled: bool,
    pub launch_widget_on_startup: bool,
    pub always_on_top: bool,
    pub show_recommendations: bool,
    pub notifications: bool,
    /// "top-left" | "top-right" | "bottom-left" | "bottom-right" | "custom"
    pub widget_position: String,
    pub custom_x: Option<i32>,
    pub custom_y: Option<i32>,
    /// 60..=100
    pub widget_opacity: u8,
}

impl Default for AsaoSettings {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            widget_enabled: true,
            launch_widget_on_startup: false,
            always_on_top: true,
            show_recommendations: true,
            notifications: true,
            widget_position: "bottom-right".to_string(),
            custom_x: None,
            custom_y: None,
            widget_opacity: 80,
        }
    }
}

pub fn get_settings_file_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("asao_settings.json"))
}

pub fn load_settings(app: &AppHandle) -> AsaoSettings {
    if let Some(path) = get_settings_file_path(app) {
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(mut parsed) = serde_json::from_str::<AsaoSettings>(&raw) {
                parsed.widget_opacity = parsed.widget_opacity.clamp(60, 100);
                return parsed;
            }
        }
    }
    AsaoSettings::default()
}

pub fn save_settings(app: &AppHandle, settings: &AsaoSettings) -> Result<(), String> {
    if let Some(path) = get_settings_file_path(app) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Positions the widget window according to the user's selected anchor or persisted
/// custom coordinates, safely clamped within the active monitor bounds.
pub fn apply_widget_window_geometry(window: &WebviewWindow, settings: &AsaoSettings) {
    let _ = window.set_always_on_top(settings.always_on_top);

    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());

    let (screen_x, screen_y, screen_w, screen_h) = if let Some(m) = monitor {
        let pos = m.position();
        let size = m.size();
        (
            pos.x,
            pos.y,
            size.width as i32,
            (size.height as i32).saturating_sub(48), // leave taskbar clearance
        )
    } else {
        (0, 0, 1920, 1032)
    };

    let win_size = window
        .outer_size()
        .unwrap_or(PhysicalSize::new(328, 412));
    let w = win_size.width as i32;
    let h = win_size.height as i32;
    let margin = 24;

    let (target_x, target_y) = match settings.widget_position.as_str() {
        "top-left" => (screen_x + margin, screen_y + margin),
        "top-right" => (screen_x + screen_w - w - margin, screen_y + margin),
        "bottom-left" => (screen_x + margin, screen_y + screen_h - h - margin),
        "custom" => {
            if let (Some(cx), Some(cy)) = (settings.custom_x, settings.custom_y) {
                (cx, cy)
            } else {
                (
                    screen_x + screen_w - w - margin,
                    screen_y + screen_h - h - margin,
                )
            }
        }
        _ => (
            screen_x + screen_w - w - margin,
            screen_y + screen_h - h - margin,
        ),
    };

    // Clamp strictly inside screen boundaries so widget is never lost off-screen
    let min_x = screen_x + 8;
    let max_x = (screen_x + screen_w - w - 8).max(min_x);
    let min_y = screen_y + 8;
    let max_y = (screen_y + screen_h - h - 8).max(min_y);

    let clamped_x = target_x.clamp(min_x, max_x);
    let clamped_y = target_y.clamp(min_y, max_y);

    let _ = window.set_position(PhysicalPosition::new(clamped_x, clamped_y));
}
