pub mod process;
pub mod startup;
pub mod storage;
pub mod system;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::analyzer::process_analyzer::ProcessAnalyzerEngine;
use crate::analyzer::recommendation_engine::RecommendationEngine;
use crate::analyzer::storage_analyzer::StorageAnalyzerEngine;
use crate::analyzer::WidgetRecommendation;
use crate::settings::AsaoSettings;
use crate::startup::scanner::StartupScanner;
use crate::tray::TrayMenuHandles;

pub const EVENT_PROCESS_SNAPSHOT: &str = "PROCESS_SNAPSHOT";
pub const EVENT_SYSTEM_METRICS: &str = "SYSTEM_METRICS";
pub const EVENT_PROCESS_ERROR: &str = "PROCESS_ERROR";
pub const EVENT_SYSTEM_UPDATE: &str = "system:update";

#[derive(Clone)]
pub struct AppMonitoringState {
    pub engine: Arc<Mutex<ProcessAnalyzerEngine>>,
    pub storage_engine: Arc<Mutex<StorageAnalyzerEngine>>,
    pub cached_startup_recs: Arc<Mutex<Vec<WidgetRecommendation>>>,
    pub cached_storage_recs: Arc<Mutex<Vec<WidgetRecommendation>>>,
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
            cached_startup_recs: Arc::new(Mutex::new(Vec::new())),
            cached_storage_recs: Arc::new(Mutex::new(Vec::new())),
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
            let startup_recs_ref = self.cached_startup_recs.clone();
            let storage_recs_ref = self.cached_storage_recs.clone();

            // Background thread to periodically refresh Startup & Storage recommendations
            let bg_engine_ref = self.engine.clone();
            let bg_storage_ref = self.storage_engine.clone();
            let bg_startup_recs = self.cached_startup_recs.clone();
            let bg_storage_recs = self.cached_storage_recs.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1200));
                loop {
                    let procs = bg_engine_ref
                        .lock()
                        .ok()
                        .and_then(|mut g| g.get_latest_or_collect().ok())
                        .map(|s| s.processes)
                        .unwrap_or_default();

                    let startup_items = StartupScanner::scan(&procs);
                    let s_recs = RecommendationEngine::evaluate_startup_recommendations(&startup_items);
                    if let Ok(mut guard) = bg_startup_recs.lock() {
                        *guard = s_recs;
                    }

                    if let Ok(st_guard) = bg_storage_ref.lock() {
                        let st_recs =
                            RecommendationEngine::evaluate_storage_recommendations(&st_guard, &procs);
                        if let Ok(mut guard) = bg_storage_recs.lock() {
                            *guard = st_recs;
                        }
                    }

                    std::thread::sleep(Duration::from_secs(30));
                }
            });

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
                        let s_recs = startup_recs_ref
                            .lock()
                            .map(|g| g.clone())
                            .unwrap_or_default();
                        let st_recs = storage_recs_ref
                            .lock()
                            .map(|g| g.clone())
                            .unwrap_or_default();

                        let widget_update = RecommendationEngine::evaluate_snapshot_with_extras(
                            &snapshot, &s_recs, &st_recs,
                        );
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
