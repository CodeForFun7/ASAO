//! Scan roots and reparse-safe folder walking.
//!
//! `ResidualScanner` roots come from the Windows known-folder API, never from
//! hard-coded user paths. Listing is shallow (depth 1 and 2 only) and never follows
//! junctions, symlinks or other reparse points.

use std::fs;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::normalize::{is_same_or_inside, path_key};
use super::{RootKind, ScanRoot};

const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

/// File extensions treated as logs, dumps or temporary data.
const LOG_LIKE_EXTENSIONS: &[&str] = &["log", "txt", "dmp", "tmp", "etl", "old"];

/// Sub-folder names whose contents count as cache or logs.
const CACHE_LIKE_DIR_WORDS: &[&str] = &["cache", "log", "crash", "dump", "temp", "tmp"];

#[cfg(target_os = "windows")]
pub fn known_roots() -> Vec<ScanRoot> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        SHGetKnownFolderPath, FOLDERID_LocalAppData, FOLDERID_LocalAppDataLow,
        FOLDERID_ProgramData, FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86,
        FOLDERID_RoamingAppData, KF_FLAG_DEFAULT,
    };

    let wanted = [
        (FOLDERID_ProgramFiles, RootKind::ProgramFiles),
        (FOLDERID_ProgramFilesX86, RootKind::ProgramFilesX86),
        (FOLDERID_RoamingAppData, RootKind::RoamingAppData),
        (FOLDERID_LocalAppData, RootKind::LocalAppData),
        (FOLDERID_LocalAppDataLow, RootKind::LocalAppDataLow),
        (FOLDERID_ProgramData, RootKind::ProgramData),
    ];

    let mut roots: Vec<ScanRoot> = Vec::new();
    for (id, kind) in wanted {
        let path = unsafe {
            match SHGetKnownFolderPath(&id, KF_FLAG_DEFAULT, HANDLE::default()) {
                Ok(p) => {
                    let s = p.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
                    s
                }
                Err(_) => String::new(),
            }
        };
        if path.is_empty() {
            continue;
        }
        let key = path_key(&path);
        // On 32-bit Windows both Program Files ids resolve to the same folder.
        if roots.iter().any(|r| path_key(&r.path.to_string_lossy()) == key) {
            continue;
        }
        roots.push(ScanRoot {
            kind,
            path: PathBuf::from(path),
        });
    }
    roots
}

#[cfg(not(target_os = "windows"))]
pub fn known_roots() -> Vec<ScanRoot> {
    Vec::new()
}

pub fn is_reparse_point(meta: &fs::Metadata) -> bool {
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Direct sub-folders of `dir`, skipping reparse points. Access errors go into `log`.
pub fn list_dirs(dir: &Path, log: &mut Vec<String>, access_denied: &mut u64) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                *access_denied += 1;
            }
            log.push(format!("Skipped {}: {}", dir.display(), e));
            return out;
        }
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_dir() || is_reparse_point(&meta) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        out.push((path, name));
    }
    out.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    out
}

pub fn system_time_ms(t: std::io::Result<std::time::SystemTime>) -> Option<u64> {
    t.ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

#[derive(Debug, Clone, Default)]
pub struct FolderStats {
    pub file_count: u64,
    pub newest_modified_ms: Option<u64>,
    /// True when every file is log-like or sits inside a cache/log/crash sub-folder.
    pub only_logs_or_caches: bool,
    /// A file newer than the cutoff was found (the walk stops early when it is).
    pub recent_found: bool,
    /// The entry budget ran out before the whole folder was seen.
    pub truncated: bool,
}

impl FolderStats {
    pub fn is_empty_or_logs(&self) -> bool {
        self.file_count == 0 || self.only_logs_or_caches
    }
}

fn dir_is_cache_like(name_lower: &str) -> bool {
    CACHE_LIKE_DIR_WORDS.iter().any(|w| name_lower.contains(w))
}

/// Walks a folder without following reparse points. When `cutoff_ms` is set the walk
/// stops at the first file modified at or after it.
pub fn folder_stats(path: &Path, cutoff_ms: Option<u64>, max_entries: u64) -> FolderStats {
    let mut stats = FolderStats {
        only_logs_or_caches: true,
        ..Default::default()
    };
    let mut stack: Vec<(PathBuf, bool)> = vec![(path.to_path_buf(), false)];
    let mut seen = 0u64;
    while let Some((dir, inside_cache_dir)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            seen += 1;
            if seen > max_entries {
                stats.truncated = true;
                return stats;
            }
            let p = entry.path();
            let Ok(meta) = fs::symlink_metadata(&p) else { continue };
            if is_reparse_point(&meta) {
                continue;
            }
            if meta.is_dir() {
                let lower = entry.file_name().to_string_lossy().to_lowercase();
                stack.push((p, inside_cache_dir || dir_is_cache_like(&lower)));
                continue;
            }
            stats.file_count += 1;
            if let Some(m) = system_time_ms(meta.modified()) {
                if stats.newest_modified_ms.map(|n| m > n).unwrap_or(true) {
                    stats.newest_modified_ms = Some(m);
                }
                if let Some(cut) = cutoff_ms {
                    if m >= cut {
                        stats.recent_found = true;
                        return stats;
                    }
                }
            }
            if !inside_cache_dir {
                let ext = p
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if !LOG_LIKE_EXTENSIONS.contains(&ext.as_str()) {
                    stats.only_logs_or_caches = false;
                }
            }
        }
    }
    stats
}

/// Microsoft redistributables that third-party apps ship next to their own files.
const BUNDLED_MICROSOFT_EXES: &[&str] = &[
    "vc_redist", "vcredist", "vcruntime", "ndp", "dotnetfx", "dotnet", "windowsdesktop-runtime",
    "aspnetcore-runtime", "dxsetup", "dxwebsetup", "msedgewebview2", "microsoftedgewebview2",
    "createdump", "msiexec",
];

/// Looks for an `.exe` or `.sys` file published by Microsoft. Windows and Microsoft
/// components (RUXIM, SQL Server runtime files, ...) often have no uninstall entry, so this
/// keeps them from ever looking like orphans. Returns the first such file name.
pub fn find_microsoft_binary(path: &Path, max_binaries: usize, max_entries: u64) -> Option<String> {
    let mut stack: Vec<PathBuf> = vec![path.to_path_buf()];
    let mut checked = 0usize;
    let mut seen = 0u64;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            seen += 1;
            if seen > max_entries || checked >= max_binaries {
                return None;
            }
            let p = entry.path();
            let Ok(meta) = fs::symlink_metadata(&p) else { continue };
            if is_reparse_point(&meta) {
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
                continue;
            }
            let ext = p
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            // DLLs are left out on purpose: many third-party apps bundle Microsoft runtime
            // DLLs (vcruntime, .NET assemblies), which says nothing about who owns the folder.
            if !matches!(ext.as_str(), "exe" | "sys") {
                continue;
            }
            let file_lower = entry.file_name().to_string_lossy().to_lowercase();
            if BUNDLED_MICROSOFT_EXES.iter().any(|prefix| file_lower.starts_with(prefix)) {
                continue;
            }
            checked += 1;
            let meta = unsafe { crate::collector::processes::extract_pe_version_metadata(&p.to_string_lossy()) };
            if meta
                .publisher
                .as_deref()
                .map(|c| c.to_lowercase().contains("microsoft"))
                .unwrap_or(false)
            {
                return Some(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    None
}

/// OneDrive folders from the environment. Deleting inside one removes the item on every synced device.
pub fn onedrive_roots() -> Vec<String> {
    ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .filter(|p| p.len() > 3)
        .map(|p| path_key(&p))
        .collect()
}

pub fn is_in_onedrive(path_key_str: &str, onedrive: &[String]) -> bool {
    onedrive.iter().any(|root| is_same_or_inside(path_key_str, root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_detect_recent_and_log_only_folders() {
        let dir = crate::residual::tests::temp_dir("roots-stats");
        fs::create_dir_all(dir.join("logs")).unwrap();
        fs::write(dir.join("a.log"), b"x").unwrap();
        fs::write(dir.join("logs").join("b.bin"), b"x").unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(90 * 86_400);
        crate::residual::tests::set_mtime(&dir.join("a.log"), old);
        crate::residual::tests::set_mtime(&dir.join("logs").join("b.bin"), old);

        let s = folder_stats(&dir, None, 10_000);
        assert_eq!(s.file_count, 2);
        assert!(s.only_logs_or_caches);

        fs::write(dir.join("settings.ini"), b"x").unwrap();
        let cutoff = crate::residual::now_ms() - 30 * 86_400_000;
        let s = folder_stats(&dir, Some(cutoff), 10_000);
        assert!(s.recent_found);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn microsoft_binaries_are_recognised() {
        let dir = crate::residual::tests::temp_dir("ms-binary");
        assert_eq!(find_microsoft_binary(&dir, 10, 1_000), None);
        fs::write(dir.join("tool.exe"), b"not a real exe").unwrap();
        assert_eq!(find_microsoft_binary(&dir, 10, 1_000), None);
        // A copy of a Windows binary carries Microsoft version info.
        if let Ok(windir) = std::env::var("WINDIR") {
            let src = Path::new(&windir).join("System32").join("cmd.exe");
            if src.exists() {
                fs::create_dir_all(dir.join("bin")).unwrap();
                fs::copy(&src, dir.join("bin").join("helper.exe")).unwrap();
                assert_eq!(find_microsoft_binary(&dir, 10, 1_000).as_deref(), Some("helper.exe"));
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
