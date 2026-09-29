pub mod process;
pub mod startup;
pub mod storage;
pub mod system;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::analyzer::process_analyzer::ProcessAnalyzerEngine;
use crate::analyzer::storage_analyzer::StorageAnalyzerEngine;
use crate::settings::AsaoSettings;
use crate::tray::TrayMenuHandles;

pub const EVENT_PROCESS_SNAPSHOT: &str = "PROCESS_SNAPSHOT";
pub const EVENT_SYSTEM_METRICS: &str = "SYSTEM_METRICS";
pub const EVENT_PROCESS_ERROR: &str = "PROCESS_ERROR";
pub const EVENT_SYSTEM_UPDATE: &str = "system:update";

#[derive(Clone)]
pub struct AppMonitoringState {
    pub engine: Arc<Mutex<ProcessAnalyzerEngine>>,
    pub storage_engine: Arc<Mutex<StorageAnalyzerEngine>>,
    pub storage_cancel_flag: Arc<AtomicBool>,
    pub storage_scanning: Arc<AtomicBool>,
    pub is_monitoring: Arc<AtomicBool>,
    pub worker_started: Arc<AtomicBool>,
    pub settings: Arc<Mutex<AsaoSettings>>,
    pub tray_handles: Arc<Mutex<Option<TrayMenuHandles>>>,
}

impl AppMonitoringState {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(ProcessAnalyzerEngine::new())),
            storage_engine: Arc::new(Mutex::new(StorageAnalyzerEngine::new())),
            storage_cancel_flag: Arc::new(AtomicBool::new(false)),
            storage_scanning: Arc::new(AtomicBool::new(false)),
            is_monitoring: Arc::new(AtomicBool::new(true)),
            worker_started: Arc::new(AtomicBool::new(false)),
            settings: Arc::new(Mutex::new(AsaoSettings::default())),
            tray_handles: Arc::new(Mutex::new(None)),
        }
    }

    pub fn ensure_worker_loop(&self, app_handle: AppHandle) {
        if self
            .worker_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let engine_ref = self.engine.clone();
            let is_monitoring_ref = self.is_monitoring.clone();
            let settings_ref = self.settings.clone();

            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(1000));

                // Ensure the widget stays in the foreground Z-order as an overlay
                let always_top = settings_ref
                    .lock()
                    .map(|g| g.always_on_top && g.widget_enabled)
                    .unwrap_or(true);
                if always_top {
                    use tauri::Manager;
                    if let Some(widget_win) = app_handle.get_webview_window("widget") {
                        if widget_win.is_visible().unwrap_or(false) {
                            crate::settings::enforce_widget_topmost(&widget_win, true);
                        }
                    }
                }

                if !is_monitoring_ref.load(Ordering::SeqCst) {
                    continue;
                }

                let result = {
                    let Ok(mut guard) = engine_ref.lock() else {
                        continue;
                    };
                    guard.collect_and_analyze()
                };

                match result {
                    Ok(snapshot) => {
                        let widget_update = ProcessAnalyzerEngine::build_widget_update(&snapshot);
                        let _ = app_handle.emit(EVENT_PROCESS_SNAPSHOT, &snapshot.processes);
                        let _ = app_handle.emit(EVENT_SYSTEM_METRICS, &snapshot.metrics);
                        let _ = app_handle.emit(EVENT_SYSTEM_UPDATE, &widget_update);
                    }
                    Err(err_msg) => {
                        let _ = app_handle.emit(EVENT_PROCESS_ERROR, err_msg);
                    }
                }
            });
        }
    }
}
