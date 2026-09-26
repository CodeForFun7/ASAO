use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::commands::AppMonitoringState;
use crate::settings::apply_widget_window_geometry;

pub struct TrayMenuHandles {
    pub show_widget_item: MenuItem<Wry>,
    pub hide_widget_item: MenuItem<Wry>,
}

pub fn setup_system_tray(
    app: &AppHandle,
    initial_widget_enabled: bool,
) -> Result<TrayMenuHandles, Box<dyn std::error::Error>> {
    let header_item = MenuItem::with_id(app, "header_asao", "ASAO", false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let show_widget_item = MenuItem::with_id(
        app,
        "show_widget",
        "Show Widget",
        initial_widget_enabled,
        None::<&str>,
    )?;
    let hide_widget_item = MenuItem::with_id(
        app,
        "hide_widget",
        "Hide Widget",
        initial_widget_enabled,
        None::<&str>,
    )?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let open_asao_item = MenuItem::with_id(app, "open_asao", "Open Asao", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "open_settings", "Settings", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit_asao", "Quit Asao", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &header_item,
            &sep1,
            &show_widget_item,
            &hide_widget_item,
            &sep2,
            &open_asao_item,
            &settings_item,
            &sep3,
            &quit_item,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("asao-tray")
        .menu(&menu)
        .tooltip("ASAO — System Telemetry & Companion")
        .show_menu_on_left_click(false)
        .on_menu_event(|app_handle, event| match event.id.as_ref() {
            "show_widget" => {
                if let Some(state) = app_handle.try_state::<AppMonitoringState>() {
                    let settings = state
                        .settings
                        .lock()
                        .map(|s| s.clone())
                        .unwrap_or_default();
                    if !settings.widget_enabled {
                        return;
                    }
                    if let Some(w) = app_handle.get_webview_window("widget") {
                        apply_widget_window_geometry(&w, &settings);
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
            }
            "hide_widget" => {
                if let Some(w) = app_handle.get_webview_window("widget") {
                    let _ = w.hide();
                }
            }
            "open_asao" => {
                if let Some(main_win) = app_handle.get_webview_window("main") {
                    let _ = main_win.unminimize();
                    let _ = main_win.show();
                    let _ = main_win.set_focus();
                }
            }
            "open_settings" => {
                if let Some(main_win) = app_handle.get_webview_window("main") {
                    let _ = main_win.unminimize();
                    let _ = main_win.show();
                    let _ = main_win.set_focus();
                }
                let _ = app_handle.emit("asao:navigate", "settings");
            }
            "quit_asao" => {
                app_handle.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app_handle = tray.app_handle();
                if let Some(main_win) = app_handle.get_webview_window("main") {
                    let _ = main_win.unminimize();
                    let _ = main_win.show();
                    let _ = main_win.set_focus();
                }
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _tray = builder.build(app)?;

    Ok(TrayMenuHandles {
        show_widget_item,
        hide_widget_item,
    })
}
