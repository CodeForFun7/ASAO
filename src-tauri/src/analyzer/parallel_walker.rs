//! Multi-threaded directory walker used when the MFT cannot be read
//! (no administrator rights, non-NTFS volume, unexpected MFT layout).
//!
//! Worker threads pull directories from a shared stack, read them, and send one
//! `ScanBatch` per directory to the calling thread, which does all aggregation.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::Duration;

use crate::analyzer::storage_analyzer::{ScanBatch, ScanDirInfo, ScanFailure, ScanFileInfo};
use crate::collector::storage::{normalize_win_path, system_time_to_ms};

struct WalkState {
    pending: Vec<PathBuf>,
    active: usize,
}

pub fn walk(
    root: &Path,
    cancel: &AtomicBool,
    on_batch: &mut dyn FnMut(ScanBatch),
) -> Result<(), ScanFailure> {
    let threads = thread::available_parallelism()
        .map(|n| n.get() * 2)
        .unwrap_or(8)
        .clamp(4, 16);

    let state = Mutex::new(WalkState {
        pending: vec![root.to_path_buf()],
        active: 0,
    });
    let cv = Condvar::new();
    let (tx, rx) = mpsc::sync_channel::<ScanBatch>(256);

    thread::scope(|s| {
        for _ in 0..threads {
            let tx = tx.clone();
            let state = &state;
            let cv = &cv;
            s.spawn(move || worker(state, cv, tx, cancel));
        }
        drop(tx);
        for batch in rx {
            on_batch(batch);
        }
    });

    if cancel.load(Ordering::Relaxed) {
        Err(ScanFailure::Cancelled)
    } else {
        Ok(())
    }
}

fn worker(state: &Mutex<WalkState>, cv: &Condvar, tx: SyncSender<ScanBatch>, cancel: &AtomicBool) {
    loop {
        let dir = {
            let mut guard = state.lock().unwrap();
            loop {
                if cancel.load(Ordering::Relaxed) {
                    cv.notify_all();
                    return;
                }
                if let Some(d) = guard.pending.pop() {
                    guard.active += 1;
                    break d;
                }
                if guard.active == 0 {
                    cv.notify_all();
                    return;
                }
                guard = cv
                    .wait_timeout(guard, Duration::from_millis(50))
                    .unwrap()
                    .0;
            }
        };

        let (batch, subdirs) = process_dir(&dir);

        {
            let mut guard = state.lock().unwrap();
            guard.pending.extend(subdirs);
            guard.active -= 1;
        }
        cv.notify_all();

        if let Some(b) = batch {
            if tx.send(b).is_err() {
                return;
            }
        }
    }
}

fn process_dir(dir: &Path) -> (Option<ScanBatch>, Vec<PathBuf>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return (None, Vec::new());
    };

    let mut batch = ScanBatch {
        dir_path: normalize_win_path(dir),
        dirs: Vec::new(),
        files: Vec::new(),
    };
    let mut subdirs = Vec::new();

    for entry in read_dir.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        // Skips symlinks and junctions so the same data is never counted twice.
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        let path_str = normalize_win_path(&path);
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().ok();
        let created_ms = meta.as_ref().and_then(|m| system_time_to_ms(m.created()));
        let modified_ms = meta.as_ref().and_then(|m| system_time_to_ms(m.modified()));
        let accessed_ms = meta.as_ref().and_then(|m| system_time_to_ms(m.accessed()));

        if ft.is_dir() {
            batch.dirs.push(ScanDirInfo {
                name,
                path: path_str,
                created_ms,
                modified_ms,
                accessed_ms,
            });
            subdirs.push(path);
        } else if ft.is_file() {
            batch.files.push(ScanFileInfo {
                name,
                path: path_str,
                size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                created_ms,
                modified_ms,
                accessed_ms,
            });
        }
    }

    (Some(batch), subdirs)
}