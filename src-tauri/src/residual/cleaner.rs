//! `ResidualCleaner`: re-checks each folder, makes sure nothing is using it, then moves
//! it to the Recycle Bin (or deletes it permanently when asked). Every result is logged.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::engine::Engine;
use super::roots::is_reparse_point;
use super::store_files::append_jsonl;
use super::{now_ms, ResidualCandidate, ResidualCategory};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CleanStatus {
    Deleted,
    WouldDelete,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanOutcome {
    pub path: String,
    pub name: String,
    pub category: Option<ResidualCategory>,
    pub size_bytes: Option<u64>,
    pub status: CleanStatus,
    pub reason: Option<String>,
    pub recycled: bool,
    pub needs_admin: bool,
    pub in_onedrive: bool,
}

impl CleanOutcome {
    pub fn skipped(item: &CleanItem, reason: impl Into<String>) -> Self {
        Self {
            path: item.path.clone(),
            name: item.name.clone(),
            category: item.category,
            size_bytes: item.size_bytes,
            status: CleanStatus::Skipped,
            reason: Some(reason.into()),
            recycled: false,
            needs_admin: item.needs_admin,
            in_onedrive: item.in_onedrive,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanItem {
    pub path: String,
    pub name: String,
    pub category: Option<ResidualCategory>,
    pub size_bytes: Option<u64>,
    pub needs_admin: bool,
    pub in_onedrive: bool,
}

impl From<&ResidualCandidate> for CleanItem {
    fn from(c: &ResidualCandidate) -> Self {
        Self {
            path: c.path.clone(),
            name: c.name.clone(),
            category: Some(c.category),
            size_bytes: c.size_bytes,
            needs_admin: c.needs_admin,
            in_onedrive: c.in_onedrive,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub dry_run: bool,
    pub permanent: bool,
    /// Only remove folders that still hold no files (the one-click empty folder clean).
    pub only_empty: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanReport {
    pub dry_run: bool,
    pub permanent: bool,
    pub freed_bytes: u64,
    pub outcomes: Vec<CleanOutcome>,
    pub admin_requested: bool,
    pub admin_declined: bool,
    pub log_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogLine<'a> {
    timestamp_ms: u64,
    action: &'a str,
    path: &'a str,
    size_bytes: Option<u64>,
    category: Option<ResidualCategory>,
    recycled: bool,
    reason: Option<&'a str>,
    elevated: bool,
}

fn collect_files(folder: &Path, max: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            let Ok(meta) = fs::symlink_metadata(&p) else { continue };
            if is_reparse_point(&meta) {
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
                if out.len() >= max {
                    return out;
                }
            }
        }
    }
    out
}

/// Names of programs holding files open inside `folder`, using the Restart Manager.
#[cfg(target_os = "windows")]
pub fn processes_using(folder: &Path) -> Result<Vec<String>, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
        RM_PROCESS_INFO,
    };

    let files = collect_files(folder, 2_000);
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let wides: Vec<Vec<u16>> = files
        .iter()
        .map(|p| p.as_os_str().encode_wide().chain(std::iter::once(0)).collect())
        .collect();
    let ptrs: Vec<PCWSTR> = wides.iter().map(|w| PCWSTR(w.as_ptr())).collect();

    unsafe {
        let mut session = 0u32;
        let mut key = [0u16; (CCH_RM_SESSION_KEY + 1) as usize];
        if RmStartSession(&mut session, 0, PWSTR(key.as_mut_ptr())) != ERROR_SUCCESS {
            return Err("Restart Manager unavailable".into());
        }
        let result = (|| {
            for chunk in ptrs.chunks(256) {
                if RmRegisterResources(session, Some(chunk), None, None) != ERROR_SUCCESS {
                    return Err("Restart Manager could not check the files".to_string());
                }
            }
            let mut needed = 0u32;
            let mut count = 0u32;
            let mut reasons = 0u32;
            let rc = RmGetList(session, &mut needed, &mut count, None, &mut reasons);
            if rc == ERROR_SUCCESS {
                return Ok(Vec::new());
            }
            if rc != ERROR_MORE_DATA {
                return Err("Restart Manager could not list programs".to_string());
            }
            let mut infos = vec![RM_PROCESS_INFO::default(); needed as usize + 4];
            count = infos.len() as u32;
            let rc = RmGetList(session, &mut needed, &mut count, Some(infos.as_mut_ptr()), &mut reasons);
            if rc != ERROR_SUCCESS {
                return Err("Restart Manager could not list programs".to_string());
            }
            let mut names: Vec<String> = infos[..count as usize]
                .iter()
                .map(|i| {
                    let end = i.strAppName.iter().position(|&c| c == 0).unwrap_or(i.strAppName.len());
                    String::from_utf16_lossy(&i.strAppName[..end])
                })
                .filter(|n| !n.is_empty())
                .collect();
            names.sort();
            names.dedup();
            Ok(names)
        })();
        let _ = RmEndSession(session);
        result
    }
}

#[cfg(not(target_os = "windows"))]
pub fn processes_using(_folder: &Path) -> Result<Vec<String>, String> {
    Err("Not supported".into())
}

/// Fallback when the Restart Manager fails: try to open each file with no sharing.
pub fn any_file_locked(folder: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    for file in collect_files(folder, 2_000) {
        let res = fs::OpenOptions::new().read(true).share_mode(0).open(&file);
        if let Err(e) = res {
            if matches!(e.raw_os_error(), Some(ERROR_SHARING_VIOLATION) | Some(ERROR_LOCK_VIOLATION)) {
                return true;
            }
        }
    }
    false
}

/// Moves a folder to the Recycle Bin, or deletes it permanently when `permanent` is set.
/// If Windows cannot recycle it (too large for the bin), it asks before deleting.
#[cfg(target_os = "windows")]
pub fn delete_folder(path: &Path, permanent: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{BOOL, HWND};
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
        FOF_WANTNUKEWARNING, FO_DELETE, SHFILEOPSTRUCTW,
    };

    // Double-null-terminated list holding one path
    let mut from: Vec<u16> = path.as_os_str().encode_wide().collect();
    from.push(0);
    from.push(0);

    let mut flags = FOF_NOCONFIRMATION.0 | FOF_SILENT.0 | FOF_NOERRORUI.0;
    if !permanent {
        flags |= FOF_ALLOWUNDO.0 | FOF_WANTNUKEWARNING.0;
    }

    unsafe {
        let com = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut op = SHFILEOPSTRUCTW {
            hwnd: HWND::default(),
            wFunc: FO_DELETE,
            pFrom: PCWSTR(from.as_ptr()),
            pTo: PCWSTR::null(),
            fFlags: flags as u16,
            fAnyOperationsAborted: BOOL(0),
            hNameMappings: std::ptr::null_mut(),
            lpszProgressTitle: PCWSTR::null(),
        };
        let rc = SHFileOperationW(&mut op);
        if com.is_ok() {
            CoUninitialize();
        }
        if rc != 0 {
            return Err(format!("Windows could not remove it (code 0x{:x})", rc));
        }
        if op.fAnyOperationsAborted.as_bool() {
            return Err("The delete was cancelled".into());
        }
    }
    if path.exists() {
        return Err("The folder is still there after the delete".into());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn delete_folder(_path: &Path, _permanent: bool) -> Result<(), String> {
    Err("Not supported".into())
}

fn log(log_path: &Path, action: &str, outcome: &CleanOutcome, elevated: bool) {
    append_jsonl(
        log_path,
        &LogLine {
            timestamp_ms: now_ms(),
            action,
            path: &outcome.path,
            size_bytes: outcome.size_bytes,
            category: outcome.category,
            recycled: outcome.recycled,
            reason: outcome.reason.as_deref(),
            elevated,
        },
    );
}

/// Checks and (unless dry run) deletes each item. Nothing is deleted without passing a fresh
/// ownership check and an in-use check first.
pub fn process_items(
    engine: &Engine,
    items: &[CleanItem],
    opts: CleanOptions,
    log_path: &Path,
    elevated: bool,
) -> Vec<CleanOutcome> {
    let mut outcomes = Vec::new();
    for item in items {
        // 1. Is the app still uninstalled and the folder still a leftover?
        let verified = if opts.only_empty {
            engine.verify_empty_for_deletion(&item.path)
        } else {
            engine.verify_for_deletion(&item.path)
        };
        if let Err(reason) = verified {
            let o = CleanOutcome::skipped(item, reason);
            log(log_path, "skipped", &o, elevated);
            outcomes.push(o);
            continue;
        }

        // 2. Is any program using files inside it?
        let folder = Path::new(&item.path);
        let in_use = match processes_using(folder) {
            Ok(names) if !names.is_empty() => Some(format!("In use by {}", names.join(", "))),
            Ok(_) => None,
            Err(_) => {
                if any_file_locked(folder) {
                    Some("A file inside is open in another program".to_string())
                } else {
                    None
                }
            }
        };
        if let Some(reason) = in_use {
            let o = CleanOutcome::skipped(item, reason);
            log(log_path, "skipped", &o, elevated);
            outcomes.push(o);
            continue;
        }

        if opts.dry_run {
            let o = CleanOutcome {
                status: CleanStatus::WouldDelete,
                reason: if item.needs_admin && !elevated {
                    Some("Needs administrator permission. Windows will ask when you remove it for real.".into())
                } else {
                    None
                },
                recycled: !opts.permanent,
                ..CleanOutcome::skipped(item, "")
            };
            log(log_path, "would-delete", &o, elevated);
            outcomes.push(o);
            continue;
        }

        // 3. Delete
        let o = match delete_folder(folder, opts.permanent) {
            Ok(()) => CleanOutcome {
                status: CleanStatus::Deleted,
                reason: None,
                recycled: !opts.permanent,
                ..CleanOutcome::skipped(item, "")
            },
            Err(e) => CleanOutcome {
                status: CleanStatus::Failed,
                reason: Some(e),
                ..CleanOutcome::skipped(item, "")
            },
        };
        log(
            log_path,
            if o.status == CleanStatus::Deleted { "deleted" } else { "failed" },
            &o,
            elevated,
        );
        outcomes.push(o);
    }
    outcomes
}

pub fn freed_bytes(outcomes: &[CleanOutcome]) -> u64 {
    outcomes
        .iter()
        .filter(|o| o.status == CleanStatus::Deleted || o.status == CleanStatus::WouldDelete)
        .map(|o| o.size_bytes.unwrap_or(0))
        .sum()
}
