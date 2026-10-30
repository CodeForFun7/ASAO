//! Tauri commands for the Unused apps tab (Storage).

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, State};

use crate::apps::actions::{
    check_folder_removable, recycle_folder, remove_shortcuts_into, remove_store_package,
    shell_open, split_command, steam_app_id_for, FolderChecks, LaunchError,
};
use crate::apps::scan::{build, ScanInputs};
use crate::apps::usage::{prefetch_last_runs, UsageStore};
use crate::apps::{
    ActionOutcome, ActionStatus, AppSizeUpdate, RemovalMethod, UninstallFinished, UnusedApp,
    UnusedAppsResult, EVENT_APPS_SIZE_UPDATE, EVENT_APPS_UNINSTALL_FINISHED,
};
use crate::commands::residual::{protected_paths, refresh_storage_recommendations, residual_files, running_exes};
use crate::commands::AppMonitoringState;
use crate::residual::engine::refresh_history;
use crate::residual::evidence::userassist_records;
use crate::residual::exclusions::ExclusionList;
use crate::residual::history::InventoryHistory;
use crate::residual::inventory::{collect_installed_apps, collect_registry_apps};
use crate::residual::normalize::path_key;
use crate::residual::references::collect_referenced_executables;
use crate::residual::roots::{is_reparse_point, known_roots};
use crate::residual::sizer;
use crate::residual::store_files::{IgnoreList, ResidualFiles};
use crate::residual::{now_ms, RootKind};

#[derive(Default)]
pub struct AppsRuntime {
    pub last: Option<UnusedAppsResult>,
    pub usage: UsageStore,
    pub usage_loaded: bool,
}

fn usage_path(files: &ResidualFiles) -> PathBuf {
    files.dir.join("app_usage.json")
}

/// Program paths running now, as written on disk.
fn running_paths(state: &AppMonitoringState) -> Vec<String> {
    state
        .engine
        .lock()
        .ok()
        .and_then(|mut g| g.get_latest_or_collect().ok())
        .map(|s| s.processes.into_iter().filter_map(|p| p.executable_path).collect())
        .unwrap_or_default()
}

fn ensure_usage_loaded(app: &AppHandle, state: &AppMonitoringState) {
    if let Ok(mut rt) = state.apps.lock() {
        if !rt.usage_loaded {
            if let Ok(files) = residual_files(app) {
                rt.usage = UsageStore::load(&usage_path(&files));
            }
            rt.usage_loaded = true;
        }
    }
}

/// Every 30 seconds, notes which programs are running; saves every 5 minutes.
pub fn start_usage_tracker(app: AppHandle, state: AppMonitoringState) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        ensure_usage_loaded(&app, &state);
        let mut ticks = 0u32;
        let mut dirty = false;
        loop {
            let running = running_paths(&state);
            if let Ok(mut rt) = state.apps.lock() {
                dirty |= rt.usage.record(running.iter().map(|s| s.as_str()), now_ms());
            }
            ticks += 1;
            if dirty && ticks % 10 == 0 {
                if let (Ok(files), Ok(rt)) = (residual_files(&app), state.apps.lock()) {
                    if rt.usage.save(&usage_path(&files)).is_ok() {
                        dirty = false;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(30));
        }
    });
}

struct Places {
    roots: Vec<String>,
    machine_roots: Vec<String>,
    broad: Vec<String>,
    path_dirs: Vec<String>,
    profile: Option<String>,
    windir: Option<String>,
}

fn places() -> Places {
    let known = known_roots();
    let mut roots: Vec<String> = known.iter().map(|r| path_key(&r.path.to_string_lossy())).collect();
    let machine_roots: Vec<String> = known
        .iter()
        .filter(|r| matches!(r.kind, RootKind::ProgramFiles | RootKind::ProgramFilesX86 | RootKind::ProgramData))
        .map(|r| path_key(&r.path.to_string_lossy()))
        .collect();
    let profile = std::env::var("USERPROFILE").ok().map(|p| path_key(&p));
    let mut broad = roots.clone();
    if let Some(p) = &profile {
        roots.push(p.clone());
        broad.push(p.clone());
        for sub in ["appdata", "downloads", "desktop", "documents", "appdata\\local\\programs"] {
            let k = format!("{}\\{}", p, sub);
            roots.push(k.clone());
            broad.push(k);
        }
    }
    for pd in machine_roots.iter().filter(|r| r.ends_with("programdata")) {
        broad.push(format!("{}\\package cache", pd));
    }
    for pf in machine_roots.iter().filter(|r| r.contains("program files")) {
        broad.push(format!("{}\\windowsapps", pf));
        broad.push(format!("{}\\common files", pf));
    }
    let windir = std::env::var("WINDIR").ok().map(|w| path_key(&w));
    let path_dirs = std::env::var("PATH")
        .map(|p| p.split(';').filter(|s| !s.trim().is_empty()).map(path_key).collect())
        .unwrap_or_default();
    Places { roots, machine_roots, broad, path_dirs, profile, windir }
}

/// Program file names (lowercase) in a folder and one level down.
fn list_exes(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![(PathBuf::from(dir), 0u8)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&p) else { continue };
            if is_reparse_point(&meta) {
                continue;
            }
            if meta.is_dir() {
                if depth < 1 {
                    stack.push((p, depth + 1));
                }
            } else {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if name.ends_with(".exe") {
                    out.push(name);
                    if out.len() >= 200 {
                        return out;
                    }
                }
            }
        }
    }
    out
}

fn scan_blocking(app: &AppHandle, state: &AppMonitoringState) -> Result<UnusedAppsResult, String> {
    let files = residual_files(app)?;
    ensure_usage_loaded(app, state);

    let inventory = collect_installed_apps();
    if inventory.failed_classes.contains(&"registry") {
        return Err("Could not read the list of installed apps.".into());
    }
    let mut history = InventoryHistory::load(&files.history());
    history.observe(&inventory.apps, &inventory.failed_classes, now_ms());
    let _ = history.save(&files.history());

    let usage = state.apps.lock().map(|rt| rt.usage.clone()).unwrap_or_default();
    let runs = userassist_records();
    let prefetch = prefetch_last_runs();
    let running = running_exes(state);
    let referenced = collect_referenced_executables().unwrap_or_default();
    let ignore = IgnoreList::load(&files.ignore_list());
    let ignored_keys = ignore.unused_app_keys();
    let exclusions = ExclusionList::new(protected_paths(app));
    let p = places();
    let exists = |x: &str| std::path::Path::new(x).exists();
    let steam = |dir: &str| steam_app_id_for(dir);

    let result = build(&ScanInputs {
        installed: &inventory.apps,
        details: &inventory.details,
        history: &history,
        usage: &usage,
        runs: &runs,
        prefetch: prefetch.as_ref(),
        running: &running,
        referenced: &referenced,
        roots: &p.roots,
        machine_roots: &p.machine_roots,
        broad: &p.broad,
        path_dirs: &p.path_dirs,
        profile: p.profile.as_deref(),
        windir: p.windir.as_deref(),
        exclusions: &exclusions,
        ignored_keys: &ignored_keys,
        exists: &exists,
        list_exes: &list_exes,
        steam_app_id: &steam,
        scan_id: now_ms(),
    });

    if let Ok(mut rt) = state.apps.lock() {
        rt.last = Some(result.clone());
    }
    start_sizing(app.clone(), state.clone(), &result);
    Ok(result)
}

/// Measures install folders that have no size from the registry, in the background.
fn start_sizing(app: AppHandle, state: AppMonitoringState, result: &UnusedAppsResult) {
    let scan_id = result.scan_id;
    let todo: Vec<(String, String)> = result
        .apps
        .iter()
        .filter(|a| a.size_bytes.is_none())
        .filter_map(|a| a.install_dir.clone().map(|d| (a.id.clone(), d)))
        .collect();
    std::thread::spawn(move || {
        let never = std::sync::atomic::AtomicBool::new(false);
        for (id, dir) in todo {
            let current = state.apps.lock().ok().and_then(|rt| rt.last.as_ref().map(|r| r.scan_id));
            if current != Some(scan_id) {
                return;
            }
            let size = sizer::measure(&PathBuf::from(&dir), &never).size_on_disk;
            if let Ok(mut rt) = state.apps.lock() {
                if let Some(r) = rt.last.as_mut() {
                    if let Some(a) = r.apps.iter_mut().find(|a| a.id == id) {
                        a.size_bytes = Some(size);
                    }
                }
            }
            let _ = app.emit(EVENT_APPS_SIZE_UPDATE, AppSizeUpdate { scan_id, id, size_bytes: size });
        }
    });
}

#[tauri::command]
pub async fn apps_scan(app: AppHandle, state: State<'_, AppMonitoringState>) -> Result<UnusedAppsResult, String> {
    let st = state.inner().clone();
    if st
        .apps_busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("A scan of apps is already running.".into());
    }
    let result = tauri::async_runtime::spawn_blocking({
        let st = st.clone();
        move || scan_blocking(&app, &st)
    })
    .await
    .map_err(|e| format!("Scan thread error: {}", e));
    st.apps_busy.store(false, Ordering::SeqCst);
    result?
}

#[tauri::command]
pub fn apps_get_last(state: State<'_, AppMonitoringState>) -> Result<Option<UnusedAppsResult>, String> {
    Ok(state.apps.lock().map_err(|_| "Apps state unavailable".to_string())?.last.clone())
}

fn forget_app(state: &AppMonitoringState, id: &str) {
    if let Ok(mut rt) = state.apps.lock() {
        if let Some(r) = rt.last.as_mut() {
            r.apps.retain(|a| a.id != id);
        }
    }
}

/// Watches Windows' installed-apps list until the app disappears (up to 20 minutes), then
/// records the removal so its leftovers are matched with high confidence.
fn watch_for_removal(app: AppHandle, state: AppMonitoringState, target: UnusedApp) {
    let Some(registry_id) = target.internal.registry_id.clone() else { return };
    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(20 * 60);
        let mut removed = false;
        while std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_secs(3));
            if let Some(apps) = collect_registry_apps() {
                if !apps.iter().any(|a| a.id == registry_id) {
                    removed = true;
                    break;
                }
            }
        }
        if removed {
            if let Ok(files) = residual_files(&app) {
                let _ = refresh_history(&files);
            }
            forget_app(&state, &target.id);
            refresh_storage_recommendations(&state);
        }
        let _ = app.emit(
            EVENT_APPS_UNINSTALL_FINISHED,
            UninstallFinished { id: target.id.clone(), name: target.name.clone(), removed },
        );
    });
}

fn launched(app: &UnusedApp, message: String, watching: bool) -> ActionOutcome {
    ActionOutcome { id: app.id.clone(), status: ActionStatus::Launched, message, watching }
}

fn from_launch_error(app: &UnusedApp, e: LaunchError) -> ActionOutcome {
    match e {
        LaunchError::Declined => ActionOutcome {
            id: app.id.clone(),
            status: ActionStatus::Declined,
            message: "Administrator permission was declined, so nothing was changed.".into(),
            watching: false,
        },
        LaunchError::Failed(msg) => ActionOutcome {
            id: app.id.clone(),
            status: ActionStatus::Failed,
            message: format!("Windows could not start it: {}", msg),
            watching: false,
        },
    }
}

fn failed(app: &UnusedApp, message: impl Into<String>) -> ActionOutcome {
    ActionOutcome { id: app.id.clone(), status: ActionStatus::Failed, message: message.into(), watching: false }
}

/// Removes one app from the latest scan, using its main method or, with `use_fallback`, its
/// second one. Only apps from the latest scan can be acted on.
#[tauri::command]
pub async fn apps_remove(
    app: AppHandle,
    state: State<'_, AppMonitoringState>,
    id: String,
    use_fallback: bool,
) -> Result<ActionOutcome, String> {
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || remove_blocking(&app, &st, &id, use_fallback))
        .await
        .map_err(|e| format!("Action thread error: {}", e))?
}

fn remove_blocking(app: &AppHandle, state: &AppMonitoringState, id: &str, use_fallback: bool) -> Result<ActionOutcome, String> {
    let target = state
        .apps
        .lock()
        .map_err(|_| "Apps state unavailable".to_string())?
        .last
        .as_ref()
        .and_then(|r| r.apps.iter().find(|a| a.id == id).cloned())
        .ok_or_else(|| "This app is not in the latest scan. Scan again.".to_string())?;

    let method = if use_fallback {
        target.fallback.ok_or_else(|| "There is no other way to remove this app.".to_string())?
    } else {
        target.method
    };

    let outcome = match method {
        RemovalMethod::Uninstaller => {
            let Some((file, params)) = target.internal.uninstall_string.as_deref().and_then(split_command) else {
                return Ok(failed(&target, "The uninstall command could not be read."));
            };
            match shell_open(&file, Some(&params)) {
                Ok(()) => {
                    watch_for_removal(app.clone(), state.clone(), target.clone());
                    launched(&target, format!("{}'s uninstaller is open. Follow its steps; ASAO will notice when it is gone.", target.name), true)
                }
                Err(e) => from_launch_error(&target, e),
            }
        }
        RemovalMethod::MsiProduct => {
            let Some(code) = target.internal.msi_product_code.clone() else {
                return Ok(failed(&target, "No Windows Installer product code."));
            };
            match shell_open("msiexec.exe", Some(&format!("/x {}", code))) {
                Ok(()) => {
                    watch_for_removal(app.clone(), state.clone(), target.clone());
                    launched(&target, format!("Windows Installer is removing {}. Follow its steps.", target.name), true)
                }
                Err(e) => from_launch_error(&target, e),
            }
        }
        RemovalMethod::WindowsSettings => match shell_open("ms-settings:appsfeatures", None) {
            Ok(()) => {
                watch_for_removal(app.clone(), state.clone(), target.clone());
                launched(&target, format!("Windows Settings is open. Find {} in the list and choose Uninstall.", target.name), true)
            }
            Err(e) => from_launch_error(&target, e),
        },
        RemovalMethod::StorePackage => {
            let Some(full) = target.internal.package_full_name.clone() else {
                return Ok(failed(&target, "No Store package name."));
            };
            match remove_store_package(&full) {
                Ok(()) => {
                    if let Ok(files) = residual_files(app) {
                        let _ = refresh_history(&files);
                    }
                    forget_app(state, &target.id);
                    refresh_storage_recommendations(state);
                    ActionOutcome {
                        id: target.id.clone(),
                        status: ActionStatus::Done,
                        message: format!("{} was removed.", target.name),
                        watching: false,
                    }
                }
                Err(e) => failed(&target, e),
            }
        }
        RemovalMethod::RemoveFolder => {
            let Some(dir) = target.install_dir.clone() else {
                return Ok(failed(&target, "ASAO does not know this app's folder."));
            };
            let running = running_exes(state);
            let referenced = collect_referenced_executables();
            let Some(referenced) = referenced else {
                return Ok(failed(&target, "Could not check services, scheduled tasks and startup entries, so nothing was removed."));
            };
            let p = places();
            let protected: Vec<String> = protected_paths(app).iter().map(|x| path_key(x)).collect();
            let checks = FolderChecks {
                running: &running,
                referenced: &referenced,
                broad: &p.broad,
                windir: p.windir.as_deref(),
                protected: &protected,
            };
            if let Err(e) = check_folder_removable(&dir, &checks) {
                return Ok(failed(&target, e));
            }
            match recycle_folder(&dir) {
                Ok(()) => {
                    if let Ok(files) = residual_files(app) {
                        let mut history = InventoryHistory::load(&files.history());
                        history.record_folder_removal(&target.name, &path_key(&dir), now_ms());
                        let _ = history.save(&files.history());
                    }
                    let shortcuts = remove_shortcuts_into(&dir);
                    forget_app(state, &target.id);
                    refresh_storage_recommendations(state);
                    ActionOutcome {
                        id: target.id.clone(),
                        status: ActionStatus::Done,
                        message: if shortcuts > 0 {
                            format!("{}'s folder and {} shortcut(s) are in the Recycle Bin.", target.name, shortcuts)
                        } else {
                            format!("{}'s folder is in the Recycle Bin.", target.name)
                        },
                        watching: false,
                    }
                }
                Err(e) => failed(&target, e),
            }
        }
        RemovalMethod::SteamUninstall => {
            let Some(app_id) = target.internal.steam_app_id.clone() else {
                return Ok(failed(&target, "Unknown Steam app id."));
            };
            match shell_open(&format!("steam://uninstall/{}", app_id), None) {
                Ok(()) => launched(&target, format!("Steam is asking to uninstall {}. Confirm it there.", target.name), false),
                Err(e) => from_launch_error(&target, e),
            }
        }
        RemovalMethod::OpenEpic => match shell_open("com.epicgames.launcher://store/library", None) {
            Ok(()) => launched(&target, format!("Epic Games Launcher is opening. Uninstall {} from your library.", target.name), false),
            Err(e) => from_launch_error(&target, e),
        },
    };
    Ok(outcome)
}

#[tauri::command]
pub fn apps_ignore(app: AppHandle, state: State<'_, AppMonitoringState>, key: String, name: String) -> Result<IgnoreList, String> {
    let files = residual_files(&app)?;
    let mut list = IgnoreList::load(&files.ignore_list());
    list.add_unused_app(&key, &name, now_ms());
    list.save(&files.ignore_list())?;
    if let Ok(mut rt) = state.apps.lock() {
        if let Some(r) = rt.last.as_mut() {
            r.apps.retain(|a| !a.key.eq_ignore_ascii_case(&key));
        }
    }
    Ok(list)
}

#[tauri::command]
pub fn apps_unignore(app: AppHandle, key: String) -> Result<IgnoreList, String> {
    let files = residual_files(&app)?;
    let mut list = IgnoreList::load(&files.ignore_list());
    list.remove_unused_app(&key);
    list.save(&files.ignore_list())?;
    Ok(list)
}
