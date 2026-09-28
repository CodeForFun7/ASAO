use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};

use crate::analyzer::storage_analyzer::{
    perform_drive_scan, read_directory_with_index, StorageAnalysisSnapshot,
    StorageDirectoryListing, StorageItem, StorageScanProgress, EVENT_STORAGE_SCAN_COMPLETE,
    EVENT_STORAGE_SCAN_PROGRESS,
};
use crate::collector::storage::{collect_storage_drives, StorageDrive};
use crate::commands::AppMonitoringState;

#[tauri::command]
pub fn get_storage_drives() -> Result<Vec<StorageDrive>, String> {
    Ok(collect_storage_drives())
}

#[tauri::command]
pub fn get_storage_snapshot(
    drive: Option<String>,
    state: State<'_, AppMonitoringState>,
) -> Result<Option<StorageAnalysisSnapshot>, String> {
    let guard = state
        .storage_engine
        .lock()
        .map_err(|_| "Failed to acquire storage engine lock".to_string())?;

    let target_drive = drive
        .or_else(|| guard.active_drive.clone())
        .unwrap_or_else(|| "C:".to_string());

    let key = target_drive.trim_end_matches('\\').to_uppercase();
    if let Some(idx) = guard.indices.get(&key) {
        if let Some(mut snap) = idx.snapshot.clone() {
            // Refresh drive capacities live
            snap.drives = collect_storage_drives();
            return Ok(Some(snap));
        }
    }

    Ok(None)
}

#[tauri::command]
pub async fn start_storage_scan(
    drive: Option<String>,
    app_handle: AppHandle,
    state: State<'_, AppMonitoringState>,
) -> Result<StorageAnalysisSnapshot, String> {
    let drives = collect_storage_drives();
    if drives.is_empty() {
        return Err("No storage devices detected.".to_string());
    }

    let target_mount = drive
        .and_then(|d| {
            let trimmed = d.trim_end_matches('\\').to_uppercase();
            drives
                .iter()
                .find(|drv| drv.drive.to_uppercase() == trimmed || drv.mount_point.to_uppercase() == d.to_uppercase())
                .map(|drv| drv.mount_point.clone())
        })
        .unwrap_or_else(|| drives[0].mount_point.clone());

    let cancel_flag = state.storage_cancel_flag.clone();
    let scanning_flag = state.storage_scanning.clone();
    let storage_engine = state.storage_engine.clone();

    cancel_flag.store(false, Ordering::SeqCst);
    scanning_flag.store(true, Ordering::SeqCst);

    let handle_clone = app_handle.clone();
    let mount_clone = target_mount.clone();

    let scan_result = tauri::async_runtime::spawn_blocking(move || {
        perform_drive_scan(&mount_clone, &cancel_flag, Some(&handle_clone))
    })
    .await
    .map_err(|e| format!("Storage scanner thread error: {}", e))?;

    scanning_flag.store(false, Ordering::SeqCst);

    match scan_result {
        Ok((index, snapshot)) => {
            let key = snapshot.selected_drive.trim_end_matches('\\').to_uppercase();
            if let Ok(mut guard) = storage_engine.lock() {
                guard.active_drive = Some(key.clone());
                guard.indices.insert(key, index);
            }

            let _ = app_handle.emit(
                EVENT_STORAGE_SCAN_PROGRESS,
                StorageScanProgress {
                    is_scanning: false,
                    drive: snapshot.selected_drive.clone(),
                    current_path: snapshot.initial_directory.path.clone(),
                    files_analyzed: snapshot.files_analyzed,
                    folders_analyzed: snapshot.folders_analyzed,
                    bytes_analyzed: snapshot.bytes_analyzed,
                    total_used_bytes: snapshot.bytes_analyzed,
                    progress_percent: 100.0,
                },
            );
            let _ = app_handle.emit(EVENT_STORAGE_SCAN_COMPLETE, &snapshot.selected_drive);

            Ok(snapshot)
        }
        Err(err) => Err(err),
    }
}

#[tauri::command]
pub fn cancel_storage_scan(state: State<'_, AppMonitoringState>) -> Result<(), String> {
    state.storage_cancel_flag.store(true, Ordering::SeqCst);
    state.storage_scanning.store(false, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn read_storage_directory(
    path: String,
    drive: Option<String>,
    state: State<'_, AppMonitoringState>,
) -> Result<StorageDirectoryListing, String> {
    let folder_sizes: HashMap<String, u64> = {
        let guard = state
            .storage_engine
            .lock()
            .map_err(|_| "Failed to acquire storage lock".to_string())?;
        let key = drive
            .or_else(|| {
                if path.len() >= 2 && path.as_bytes()[1] == b':' {
                    Some(path[..2].to_uppercase())
                } else {
                    guard.active_drive.clone()
                }
            })
            .unwrap_or_else(|| "C:".to_string())
            .trim_end_matches('\\')
            .to_uppercase();

        guard
            .indices
            .get(&key)
            .map(|idx| idx.folder_sizes.clone())
            .unwrap_or_default()
    };

    let listing = tauri::async_runtime::spawn_blocking(move || {
        read_directory_with_index(&path, &folder_sizes)
    })
    .await
    .map_err(|e| format!("Directory read thread error: {}", e))?;

    Ok(listing)
}

#[tauri::command]
pub async fn search_storage_items(
    query: String,
    drive: Option<String>,
    current_path: Option<String>,
    state: State<'_, AppMonitoringState>,
) -> Result<Vec<StorageItem>, String> {
    let trimmed = query.trim().to_lowercase();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let (indexed_items, folder_sizes) = {
        let guard = state
            .storage_engine
            .lock()
            .map_err(|_| "Failed to acquire storage lock".to_string())?;
        let key = drive
            .or_else(|| guard.active_drive.clone())
            .unwrap_or_else(|| "C:".to_string())
            .trim_end_matches('\\')
            .to_uppercase();

        if let Some(idx) = guard.indices.get(&key) {
            (idx.indexed_items.clone(), idx.folder_sizes.clone())
        } else {
            (Vec::new(), HashMap::new())
        }
    };

    let results = tauri::async_runtime::spawn_blocking(move || {
        let mut matched: Vec<StorageItem> = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();

        // 1. If current_path is provided, search its direct children first
        if let Some(ref cur_p) = current_path {
            let dir_list = read_directory_with_index(cur_p, &folder_sizes);
            for item in dir_list.items {
                if matches_search_query(&item, &trimmed) && seen_paths.insert(item.path.to_lowercase()) {
                    matched.push(item);
                }
            }
        }

        // 2. Search across drive-wide storage index
        for item in indexed_items {
            if matches_search_query(&item, &trimmed) && seen_paths.insert(item.path.to_lowercase()) {
                matched.push(item);
                if matched.len() >= 250 {
                    break;
                }
            }
        }

        matched.sort_by(|a, b| b.size.cmp(&a.size));
        matched
    })
    .await
    .map_err(|e| format!("Search error: {}", e))?;

    Ok(results)
}

fn matches_search_query(item: &StorageItem, q: &str) -> bool {
    let q_clean = q.trim_start_matches('.');
    item.name.to_lowercase().contains(q)
        || item.path.to_lowercase().contains(q)
        || (!item.extension.is_empty() && item.extension.to_lowercase() == q_clean)
        || item.item_type.to_lowercase().contains(q)
}

#[tauri::command]
pub fn open_storage_location(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() {
        return Err("Target path no longer exists on disk.".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        if p.is_file() {
            Command::new("explorer")
                .arg(format!("/select,{}", path))
                .spawn()
                .map_err(|e| format!("Failed to open Explorer: {}", e))?;
        } else {
            Command::new("explorer")
                .arg(&path)
                .spawn()
                .map_err(|e| format!("Failed to open Explorer: {}", e))?;
        }
    }

    Ok(())
}
