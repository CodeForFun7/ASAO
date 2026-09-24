pub mod process;
pub mod system;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::analyzer::process_analyzer::ProcessAnalyzerEngine;

pub const EVENT_PROCESS_SNAPSHOT: &str = "PROCESS_SNAPSHOT";
pub const EVENT_SYSTEM_METRICS: &str = "SYSTEM_METRICS";
pub const EVENT_PROCESS_ERROR: &str = "PROCESS_ERROR";

#[derive(Clone)]
pub struct AppMonitoringState {
    pub engine: Arc<Mutex<ProcessAnalyzerEngine>>,
    pub is_monitoring: Arc<AtomicBool>,
    pub worker_started: Arc<AtomicBool>,
}

impl AppMonitoringState {
    pub fn new() -> Self {
        Self {
            engine: Arc::new(Mutex::new(ProcessAnalyzerEngine::new())),
            is_monitoring: Arc::new(AtomicBool::new(true)),
            worker_started: Arc::new(AtomicBool::new(false)),
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

            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(1000));

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
                        let _ = app_handle.emit(EVENT_PROCESS_SNAPSHOT, &snapshot.processes);
                        let _ = app_handle.emit(EVENT_SYSTEM_METRICS, &snapshot.metrics);
                    }
                    Err(err_msg) => {
                        let _ = app_handle.emit(EVENT_PROCESS_ERROR, err_msg);
                    }
                }
            });
        }
    }
}
