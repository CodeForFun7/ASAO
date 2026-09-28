use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

use crate::collector::storage::{
    collect_storage_drives, normalize_win_path, system_time_to_ms, StorageDrive,
};

pub const EVENT_STORAGE_SCAN_PROGRESS: &str = "STORAGE_SCAN_PROGRESS";
pub const EVENT_STORAGE_SCAN_COMPLETE: &str = "STORAGE_SCAN_COMPLETE";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageCategory {
    SYSTEM,
    APPLICATION,
    USER,
    CACHE,
    TEMPORARY,
    UNKNOWN,
}

impl StorageCategory {
    pub fn label(&self) -> &'static str {
        match self {
            StorageCategory::SYSTEM => "System",
            StorageCategory::APPLICATION => "Applications",
            StorageCategory::USER => "User Files",
            StorageCategory::CACHE => "Cache",
            StorageCategory::TEMPORARY => "Temporary Files",
            StorageCategory::UNKNOWN => "Other / Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorageImportance {
    CRITICAL,
    IMPORTANT,
    NORMAL,
    LOW,
    UNKNOWN,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageItem {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub item_type: String,
    pub extension: String,
    pub category: StorageCategory,
    pub importance: StorageImportance,
    pub created_ms: Option<u64>,
    pub modified_ms: Option<u64>,
    pub accessed_ms: Option<u64>,
    pub analysis_explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCategorySlice {
    pub category: StorageCategory,
    pub label: String,
    pub bytes: u64,
    pub percentage: f64,
    pub item_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInsightAffectedItem {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    pub category: StorageCategory,
    pub importance: StorageImportance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInsight {
    pub id: String,
    pub insight_type: String,
    pub title: String,
    pub description: String,
    pub size: u64,
    pub affected_items: Vec<StorageInsightAffectedItem>,
    pub importance: StorageImportance,
    pub action: String,
    pub target_path: Option<String>,
    pub target_category: Option<StorageCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageScanProgress {
    pub is_scanning: bool,
    pub drive: String,
    pub current_path: String,
    pub files_analyzed: u64,
    pub folders_analyzed: u64,
    pub bytes_analyzed: u64,
    pub total_used_bytes: u64,
    pub progress_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDirectoryListing {
    pub path: String,
    pub parent_path: Option<String>,
    pub items: Vec<StorageItem>,
    pub permission_denied: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageAnalysisSnapshot {
    pub selected_drive: String,
    pub drives: Vec<StorageDrive>,
    pub distribution: Vec<StorageCategorySlice>,
    pub insights: Vec<StorageInsight>,
    pub largest_folders: Vec<StorageItem>,
    pub largest_files: Vec<StorageItem>,
    pub discovered_extensions: Vec<String>,
    pub initial_directory: StorageDirectoryListing,
    pub files_analyzed: u64,
    pub folders_analyzed: u64,
    pub bytes_analyzed: u64,
    pub scan_timestamp_ms: u64,
}

#[derive(Default)]
pub struct DriveStorageIndex {
    pub folder_sizes: HashMap<String, u64>,
    pub indexed_items: Vec<StorageItem>,
    pub snapshot: Option<StorageAnalysisSnapshot>,
}

pub struct StorageAnalyzerEngine {
    pub indices: HashMap<String, DriveStorageIndex>,
    pub active_drive: Option<String>,
}

impl StorageAnalyzerEngine {
    pub fn new() -> Self {
        Self {
            indices: HashMap::new(),
            active_drive: None,
        }
    }
}

pub fn normalize_key(path: &str) -> String {
    let mut s = path.replace('/', "\\");
    if s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    s.to_lowercase()
}

pub fn classify_storage_item(
    path_str: &str,
    name: &str,
    is_dir: bool,
    ext: &str,
    size: u64,
    modified_ms: Option<u64>,
) -> (StorageCategory, StorageImportance, String, String) {
    let lower_path = path_str.to_lowercase();
    let lower_name = name.to_lowercase();
    let lower_ext = ext.to_lowercase();

    // Determine human-readable Item Type
    let item_type = if is_dir {
        "Folder".to_string()
    } else {
        match lower_ext.as_str() {
            "exe" | "msi" | "bat" | "cmd" | "ps1" | "com" => "Application".to_string(),
            "dll" | "sys" | "drv" | "ocx" | "mui" | "cat" => "System Binary".to_string(),
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" | "cab" => {
                "Archive".to_string()
            }
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" => "Video".to_string(),
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" => "Audio".to_string(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "psd" => {
                "Image".to_string()
            }
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "rtf"
            | "csv" => "Document".to_string(),
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "c" | "cpp" | "h" | "cs" | "go"
            | "java" | "json" | "yaml" | "yml" | "toml" | "html" | "css" | "sql" => {
                "Source / Config".to_string()
            }
            "tmp" | "temp" | "bak" | "old" | "dmp" | "log" | "etl" => "Temporary / Log".to_string(),
            "db" | "sqlite" | "sqlite3" | "mdb" | "ldb" | "idx" | "pak" => {
                "Data / Package".to_string()
            }
            "" => "File".to_string(),
            other => format!("{} File", other.to_uppercase()),
        }
    };

    // 1. Check TEMPORARY patterns
    let is_temp_path = lower_path.contains("\\temp\\")
        || lower_path.ends_with("\\temp")
        || lower_path.contains("\\tmp\\")
        || lower_path.ends_with("\\tmp")
        || lower_path.contains("\\windows\\softwaredistribution\\download")
        || lower_path.contains("\\windows\\prefetch")
        || lower_path.contains("\\$recycle.bin")
        || lower_name.starts_with('~');

    let is_temp_ext = matches!(
        lower_ext.as_str(),
        "tmp" | "temp" | "dmp" | "bak" | "old" | "crdownload" | "part"
    );

    if is_temp_path || is_temp_ext {
        let explanation = if lower_path.contains("\\$recycle.bin") {
            "Located in the Windows Recycle Bin. These items occupy disk space until permanently emptied.".to_string()
        } else if lower_path.contains("softwaredistribution") {
            "Windows Update staging package. Used temporarily during OS updates and safe for Windows Cleanup.".to_string()
        } else {
            "Temporary working data or residual log/staging file. Usually safe to remove when applications are closed.".to_string()
        };
        return (
            StorageCategory::TEMPORARY,
            StorageImportance::LOW,
            item_type,
            explanation,
        );
    }

    // 2. Check CACHE patterns
    let is_cache_path = lower_path.contains("\\cache\\")
        || lower_path.ends_with("\\cache")
        || lower_path.contains("\\caches\\")
        || lower_path.contains("\\code cache")
        || lower_path.contains("\\gpucache")
        || lower_path.contains("\\shadercache")
        || lower_path.contains("\\dxcache")
        || lower_path.contains("\\inetcache")
        || lower_path.contains("\\crashpad")
        || lower_path.contains("\\.npm")
        || lower_path.contains("\\.cargo\\registry")
        || lower_path.contains("\\.gradle\\caches")
        || lower_path.contains("\\pip\\cache")
        || lower_path.contains("\\pnpm\\store")
        || lower_path.contains("\\nuget\\packages");

    if is_cache_path {
        let explanation = "Application or package manager cache directory. Speeds up runtime lookups and builds; can be regenerated automatically if cleared.".to_string();
        return (
            StorageCategory::CACHE,
            StorageImportance::LOW,
            item_type,
            explanation,
        );
    }

    // 3. Check SYSTEM patterns
    let is_system_root_file = matches!(
        lower_name.as_str(),
        "pagefile.sys" | "hiberfil.sys" | "swapfile.sys" | "bootmgr" | "ntldr"
    );
    let is_system_dir = lower_path.contains(":\\windows")
        || lower_path.contains(":\\system volume information")
        || lower_path.contains(":\\recovery")
        || lower_path.contains(":\\$extend")
        || lower_path.contains(":\\perflogs")
        || lower_path.contains(":\\efi");

    if is_system_root_file || is_system_dir {
        let explanation = if is_system_root_file {
            "Core Windows virtual memory or power management system file. Managed directly by the OS kernel and critical for system stability.".to_string()
        } else if lower_path.contains("\\system32") || lower_path.contains("\\winsxs") {
            "Essential Windows operating system directory containing core binaries, drivers, and side-by-side component assemblies. Do not modify or delete.".to_string()
        } else {
            "Operating system directory or component required by Microsoft Windows. Modifying these files may affect OS boot or stability.".to_string()
        };
        return (
            StorageCategory::SYSTEM,
            StorageImportance::CRITICAL,
            item_type,
            explanation,
        );
    }

    // 4. Check APPLICATION patterns
    let is_app_dir = lower_path.contains(":\\program files")
        || lower_path.contains(":\\program files (x86)")
        || lower_path.contains(":\\programdata")
        || lower_path.contains("\\appdata\\local\\programs")
        || lower_path.contains("\\appdata\\roaming")
        || lower_path.contains("\\appdata\\local")
        || lower_path.contains("\\steamapps")
        || lower_path.contains("\\epic games")
        || lower_path.contains("\\windowsapps");

    if is_app_dir {
        let importance = if lower_path.contains(":\\program files")
            || lower_path.contains("\\appdata\\roaming")
        {
            StorageImportance::IMPORTANT
        } else {
            StorageImportance::NORMAL
        };
        let explanation = if lower_path.contains("\\appdata\\") {
            "Application configuration, state, or local runtime storage located in the user's AppData profile.".to_string()
        } else if lower_path.contains(":\\programdata") {
            "Shared machine-wide application data and service configurations.".to_string()
        } else {
            "Installed software package or application runtime directory. Remove only via official application uninstaller.".to_string()
        };
        return (
            StorageCategory::APPLICATION,
            importance,
            item_type,
            explanation,
        );
    }

    // 5. Check USER patterns
    let is_user_dir = lower_path.contains(":\\users")
        || lower_path.contains("\\documents")
        || lower_path.contains("\\downloads")
        || lower_path.contains("\\desktop")
        || lower_path.contains("\\pictures")
        || lower_path.contains("\\videos")
        || lower_path.contains("\\music")
        || lower_path.contains("\\onedrive")
        || lower_path.contains("\\coding")
        || lower_path.contains("\\projects")
        || lower_path.contains("\\source")
        || lower_path.contains("\\repos");

    if is_user_dir {
        // Check if inside Downloads or node_modules/build output
        if lower_path.contains("\\node_modules")
            || lower_path.ends_with("\\target")
            || lower_path.contains("\\target\\debug")
            || lower_path.contains("\\target\\release")
        {
            let explanation = "Development build dependency or compiled artifact directory. Can consume substantial disk space and be recreated via your project's build/package tool.".to_string();
            return (
                StorageCategory::CACHE,
                StorageImportance::LOW,
                item_type,
                explanation,
            );
        }

        if lower_path.contains("\\downloads") {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let age_days = modified_ms
                .map(|m| now_ms.saturating_sub(m) / (1000 * 60 * 60 * 24))
                .unwrap_or(0);

            let importance = if age_days > 30 || matches!(lower_ext.as_str(), "exe" | "msi" | "zip" | "iso") {
                StorageImportance::LOW
            } else {
                StorageImportance::NORMAL
            };
            let explanation = if age_days > 14 {
                format!(
                    "Downloaded item in your user profile (last modified {} days ago). Verify if still needed; installers and archives in Downloads can often be reclaimed.",
                    age_days
                )
            } else {
                "User file stored in the Downloads folder. Review before deleting.".to_string()
            };
            return (StorageCategory::USER, importance, item_type, explanation);
        }

        let importance = if size > 500 * 1024 * 1024 {
            StorageImportance::NORMAL
        } else {
            StorageImportance::IMPORTANT
        };
        let explanation = "Personal user directory or workspace file. Contains user documents, media, or project assets.".to_string();
        return (StorageCategory::USER, importance, item_type, explanation);
    }

    // 6. Fallback based on extension or non-C drive user workspace
    if !is_dir {
        match lower_ext.as_str() {
            "sys" | "dll" | "drv" => {
                return (
                    StorageCategory::SYSTEM,
                    StorageImportance::CRITICAL,
                    item_type,
                    "Binary library or system driver component.".to_string(),
                );
            }
            "exe" | "msi" => {
                return (
                    StorageCategory::APPLICATION,
                    StorageImportance::NORMAL,
                    item_type,
                    "Executable program or installer binary.".to_string(),
                );
            }
            "mp4" | "mkv" | "mov" | "zip" | "7z" | "rar" | "pdf" | "docx" | "png" | "jpg" => {
                return (
                    StorageCategory::USER,
                    StorageImportance::NORMAL,
                    item_type,
                    "User media, archive, or document file stored on this volume.".to_string(),
                );
            }
            _ => {}
        }
    }

    // Check if non-system drive root folder
    if !lower_path.starts_with("c:") {
        return (
            StorageCategory::USER,
            StorageImportance::NORMAL,
            item_type,
            "Directory or file located on secondary storage volume.".to_string(),
        );
    }

    (
        StorageCategory::UNKNOWN,
        StorageImportance::UNKNOWN,
        item_type,
        "Unclassified filesystem entry on this volume. Inspect contents and last modification date before making changes.".to_string(),
    )
}

pub fn compute_quick_dir_size(path: &Path, max_entries: usize) -> u64 {
    let mut total = 0u64;
    let mut visited = 0usize;
    let mut queue = VecDeque::new();
    queue.push_back((path.to_path_buf(), 0u8));

    while let Some((curr, depth)) = queue.pop_front() {
        if visited >= max_entries {
            break;
        }
        let Ok(read_dir) = fs::read_dir(&curr) else {
            continue;
        };
        for entry_res in read_dir {
            if visited >= max_entries {
                break;
            }
            let Ok(entry) = entry_res else {
                continue;
            };
            visited += 1;
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_file() {
                total = total.saturating_add(meta.len());
            } else if meta.is_dir() && depth < 3 {
                let file_type = entry.file_type();
                let is_symlink = file_type.map(|ft| ft.is_symlink()).unwrap_or(false);
                if !is_symlink {
                    queue.push_back((entry.path(), depth + 1));
                }
            }
        }
    }
    total
}

pub fn read_directory_with_index(
    dir_path: &str,
    folder_sizes: &HashMap<String, u64>,
) -> StorageDirectoryListing {
    let path_obj = Path::new(dir_path);
    let clean_path = normalize_win_path(path_obj);

    let parent_path = path_obj.parent().and_then(|p| {
        let p_str = normalize_win_path(p);
        if p_str.is_empty() || p_str == clean_path {
            None
        } else {
            Some(p_str)
        }
    });

    let read_res = fs::read_dir(path_obj);
    let entries = match read_res {
        Ok(rd) => rd,
        Err(err) => {
            let is_perm = err.kind() == ErrorKind::PermissionDenied;
            return StorageDirectoryListing {
                path: clean_path,
                parent_path,
                items: Vec::new(),
                permission_denied: is_perm,
                error_message: Some(err.to_string()),
            };
        }
    };

    let mut items = Vec::new();
    for entry_res in entries {
        let Ok(entry) = entry_res else {
            continue;
        };
        let entry_path = entry.path();
        let entry_path_str = normalize_win_path(&entry_path);
        let name = entry.file_name().to_string_lossy().to_string();

        let meta_opt = entry.metadata().ok();
        let is_dir = meta_opt.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let ext = if is_dir {
            String::new()
        } else {
            entry_path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        };

        let created_ms = meta_opt.as_ref().and_then(|m| system_time_to_ms(m.created()));
        let modified_ms = meta_opt.as_ref().and_then(|m| system_time_to_ms(m.modified()));
        let accessed_ms = meta_opt.as_ref().and_then(|m| system_time_to_ms(m.accessed()));

        let size = if is_dir {
            let key = normalize_key(&entry_path_str);
            if let Some(&indexed_sz) = folder_sizes.get(&key) {
                indexed_sz
            } else {
                compute_quick_dir_size(&entry_path, 1800)
            }
        } else {
            meta_opt.as_ref().map(|m| m.len()).unwrap_or(0)
        };

        let (category, importance, item_type, analysis_explanation) =
            classify_storage_item(&entry_path_str, &name, is_dir, &ext, size, modified_ms);

        items.push(StorageItem {
            name,
            path: entry_path_str,
            is_dir,
            size,
            item_type,
            extension: ext,
            category,
            importance,
            created_ms,
            modified_ms,
            accessed_ms,
            analysis_explanation,
        });
    }

    // Default sort by size descending
    items.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));

    StorageDirectoryListing {
        path: clean_path,
        parent_path,
        items,
        permission_denied: false,
        error_message: None,
    }
}

pub fn perform_drive_scan(
    drive_mount: &str,
    cancel_flag: &Arc<AtomicBool>,
    app_handle: Option<&AppHandle>,
) -> Result<(DriveStorageIndex, StorageAnalysisSnapshot), String> {
    let drives = collect_storage_drives();
    let target_drive = drives
        .iter()
        .find(|d| {
            d.mount_point.eq_ignore_ascii_case(drive_mount)
                || d.drive.eq_ignore_ascii_case(drive_mount.trim_end_matches('\\'))
        })
        .cloned()
        .or_else(|| drives.first().cloned())
        .ok_or_else(|| "No storage devices detected.".to_string())?;

    let root_path = PathBuf::from(&target_drive.mount_point);
    let root_norm = normalize_win_path(&root_path);
    let root_key = normalize_key(&root_norm);

    let mut folder_sizes: HashMap<String, u64> = HashMap::new();
    let mut folder_meta: HashMap<String, (String, String, Option<u64>, Option<u64>, Option<u64>)> =
        HashMap::new();
    let mut category_bytes: HashMap<StorageCategory, u64> = HashMap::new();
    let mut category_counts: HashMap<StorageCategory, u64> = HashMap::new();
    let mut extension_set: HashSet<String> = HashSet::new();

    let mut largest_files: Vec<StorageItem> = Vec::new();
    let mut indexed_items: Vec<StorageItem> = Vec::new();

    let mut files_analyzed: u64 = 0;
    let mut folders_analyzed: u64 = 0;
    let mut bytes_analyzed: u64 = 0;

    // Seed priority traversal queue so user profile, AppData, Downloads, Program Files,
    // ProgramData, and root directories are thoroughly analyzed within a fast scan budget.
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    let mut visited_dirs: HashSet<String> = HashSet::new();

    // Enqueue priority directories on this drive first if they exist
    let mut priority_seeds: Vec<PathBuf> = Vec::new();
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let up_path = PathBuf::from(&user_profile);
        if up_path
            .to_string_lossy()
            .to_lowercase()
            .starts_with(&root_key)
        {
            priority_seeds.push(up_path.join("Downloads"));
            priority_seeds.push(up_path.join("Documents"));
            priority_seeds.push(up_path.join("Desktop"));
            priority_seeds.push(up_path.join("Videos"));
            priority_seeds.push(up_path.join("Pictures"));
            priority_seeds.push(up_path.join("AppData\\Local\\Temp"));
            priority_seeds.push(up_path.join("AppData\\Local"));
            priority_seeds.push(up_path.join("AppData\\Roaming"));
            priority_seeds.push(up_path);
        }
    }
    priority_seeds.push(root_path.join("Users"));
    priority_seeds.push(root_path.join("Program Files"));
    priority_seeds.push(root_path.join("Program Files (x86)"));
    priority_seeds.push(root_path.join("ProgramData"));
    priority_seeds.push(root_path.join("Windows\\Temp"));
    priority_seeds.push(root_path.join("Windows\\SoftwareDistribution\\Download"));
    priority_seeds.push(root_path.join("Windows"));
    priority_seeds.push(root_path.clone());

    // First, always read root directory entries so top-level folders and root files (like pagefile.sys/hiberfil.sys) are captured
    if let Ok(root_entries) = fs::read_dir(&root_path) {
        for entry_res in root_entries.flatten() {
            let p = entry_res.path();
            if p.is_dir() {
                priority_seeds.insert(0, p);
            }
        }
    }

    for seed in priority_seeds {
        if seed.exists() {
            let k = normalize_key(&normalize_win_path(&seed));
            if visited_dirs.insert(k) {
                let depth = seed
                    .strip_prefix(&root_path)
                    .map(|rel| rel.components().count())
                    .unwrap_or(0);
                queue.push_back((seed, depth));
            }
        }
    }

    let max_entries_budget: u64 = 95_000;
    let max_depth: usize = 7;
    let mut last_emit = Instant::now();

    while let Some((curr_dir, depth)) = queue.pop_front() {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err("Scan cancelled by user.".to_string());
        }

        if files_analyzed + folders_analyzed >= max_entries_budget {
            break;
        }

        let curr_dir_str = normalize_win_path(&curr_dir);
        folders_analyzed += 1;

        if last_emit.elapsed().as_millis() >= 90 {
            if let Some(handle) = app_handle {
                let entry_ratio =
                    (files_analyzed + folders_analyzed) as f64 / max_entries_budget as f64;
                let byte_ratio = if target_drive.used_capacity > 0 {
                    bytes_analyzed as f64 / target_drive.used_capacity as f64
                } else {
                    0.0
                };
                let pct = ((entry_ratio.max(byte_ratio) * 95.0).clamp(2.0, 96.0) * 10.0).round()
                    / 10.0;
                let _ = handle.emit(
                    EVENT_STORAGE_SCAN_PROGRESS,
                    StorageScanProgress {
                        is_scanning: true,
                        drive: target_drive.drive.clone(),
                        current_path: curr_dir_str.clone(),
                        files_analyzed,
                        folders_analyzed,
                        bytes_analyzed,
                        total_used_bytes: target_drive.used_capacity,
                        progress_percent: pct,
                    },
                );
            }
            last_emit = Instant::now();
        }

        let Ok(rd) = fs::read_dir(&curr_dir) else {
            continue;
        };

        let mut dir_direct_bytes = 0u64;

        for entry_res in rd {
            if cancel_flag.load(Ordering::SeqCst) {
                return Err("Scan cancelled by user.".to_string());
            }

            let Ok(entry) = entry_res else {
                continue;
            };
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }

            let entry_path = entry.path();
            let entry_path_str = normalize_win_path(&entry_path);
            let name = entry.file_name().to_string_lossy().to_string();
            let meta_opt = entry.metadata().ok();

            let created_ms = meta_opt.as_ref().and_then(|m| system_time_to_ms(m.created()));
            let modified_ms = meta_opt.as_ref().and_then(|m| system_time_to_ms(m.modified()));
            let accessed_ms = meta_opt.as_ref().and_then(|m| system_time_to_ms(m.accessed()));

            if ft.is_dir() {
                let key = normalize_key(&entry_path_str);
                folder_meta
                    .entry(key.clone())
                    .or_insert_with(|| (name.clone(), entry_path_str.clone(), created_ms, modified_ms, accessed_ms));

                if depth < max_depth && visited_dirs.insert(key) {
                    // Prioritize non-WinSxS directories so we don't spend the whole budget inside WinSxS
                    let lower_p = entry_path_str.to_lowercase();
                    if !lower_p.contains("\\windows\\winsxs\\")
                        && !lower_p.contains("\\windows\\servicing\\")
                    {
                        queue.push_back((entry_path, depth + 1));
                    }
                }
            } else if ft.is_file() {
                let sz = meta_opt.as_ref().map(|m| m.len()).unwrap_or(0);
                files_analyzed += 1;
                bytes_analyzed = bytes_analyzed.saturating_add(sz);
                dir_direct_bytes = dir_direct_bytes.saturating_add(sz);

                let ext = entry_path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();

                if !ext.is_empty() && ext.len() <= 10 && extension_set.len() < 60 {
                    extension_set.insert(ext.clone());
                }

                let (cat, imp, item_type, explanation) =
                    classify_storage_item(&entry_path_str, &name, false, &ext, sz, modified_ms);

                *category_bytes.entry(cat).or_insert(0) += sz;
                *category_counts.entry(cat).or_insert(0) += 1;

                // Keep notable files in search index & largest files tracker
                if sz >= 2 * 1024 * 1024 || indexed_items.len() < 4_500 {
                    let item = StorageItem {
                        name,
                        path: entry_path_str,
                        is_dir: false,
                        size: sz,
                        item_type,
                        extension: ext,
                        category: cat,
                        importance: imp,
                        created_ms,
                        modified_ms,
                        accessed_ms,
                        analysis_explanation: explanation,
                    };

                    if sz >= 5 * 1024 * 1024 {
                        largest_files.push(item.clone());
                        if largest_files.len() > 220 {
                            largest_files.sort_by(|a, b| b.size.cmp(&a.size));
                            largest_files.truncate(120);
                        }
                    }

                    if indexed_items.len() < 8_000 {
                        indexed_items.push(item);
                    }
                }
            }
        }

        // Roll up dir_direct_bytes to curr_dir and all its ancestors up to root_path
        if dir_direct_bytes > 0 {
            let mut anc_opt = Some(curr_dir.as_path());
            while let Some(anc) = anc_opt {
                let anc_str = normalize_win_path(anc);
                let anc_key = normalize_key(&anc_str);
                *folder_sizes.entry(anc_key.clone()).or_insert(0) += dir_direct_bytes;

                if anc_key == root_key {
                    break;
                }
                anc_opt = anc.parent();
            }
        }
    }

    // Account for OS-level protected/unindexed space on the drive (e.g., WinSxS hardlinks, System Volume Information, MFT)
    if target_drive.used_capacity > bytes_analyzed {
        let unindexed_bytes = target_drive.used_capacity - bytes_analyzed;
        if target_drive.drive.eq_ignore_ascii_case("C:") {
            // On the Windows boot volume, unindexed kernel/protected space belongs to SYSTEM (WinSxS, System Volume Info, MFT) and UNKNOWN
            let sys_portion = (unindexed_bytes as f64 * 0.68) as u64;
            let other_portion = unindexed_bytes.saturating_sub(sys_portion);
            *category_bytes.entry(StorageCategory::SYSTEM).or_insert(0) += sys_portion;
            *category_bytes.entry(StorageCategory::UNKNOWN).or_insert(0) += other_portion;

            let win_key = normalize_key(&format!("{}Windows", target_drive.mount_point));
            let cur_win = folder_sizes.get(&win_key).copied().unwrap_or(0);
            if cur_win < sys_portion {
                folder_sizes.insert(win_key, cur_win + (sys_portion / 2));
            }
        } else {
            *category_bytes.entry(StorageCategory::UNKNOWN).or_insert(0) += unindexed_bytes;
        }
    }

    // Build Storage Distribution slices across all 6 categories
    let total_dist_bytes: u64 = category_bytes.values().sum::<u64>().max(1);
    let ordered_categories = [
        StorageCategory::SYSTEM,
        StorageCategory::APPLICATION,
        StorageCategory::USER,
        StorageCategory::CACHE,
        StorageCategory::TEMPORARY,
        StorageCategory::UNKNOWN,
    ];

    let distribution: Vec<StorageCategorySlice> = ordered_categories
        .iter()
        .map(|&cat| {
            let b = category_bytes.get(&cat).copied().unwrap_or(0);
            let cnt = category_counts.get(&cat).copied().unwrap_or(0);
            let pct = ((b as f64 / total_dist_bytes as f64) * 1000.0).round() / 10.0;
            StorageCategorySlice {
                category: cat,
                label: cat.label().to_string(),
                bytes: b,
                percentage: pct,
                item_count: cnt,
            }
        })
        .collect();

    // Build Largest Folders list (meaningful top-level and 2nd-level folders)
    let mut folder_items: Vec<StorageItem> = Vec::new();
    for (key, &sz) in &folder_sizes {
        if sz == 0 || key == &root_key {
            continue;
        }
        if let Some((name, full_path, c_ms, m_ms, a_ms)) = folder_meta.get(key) {
            let rel_depth = Path::new(full_path)
                .strip_prefix(&root_path)
                .map(|r| r.components().count())
                .unwrap_or(99);

            // Include depth 1 and 2 folders (e.g. C:\Users, C:\Program Files, C:\Windows, C:\Users\<user>\AppData, etc.)
            if rel_depth >= 1 && rel_depth <= 3 {
                let (cat, imp, item_type, explanation) =
                    classify_storage_item(full_path, name, true, "", sz, *m_ms);
                let item = StorageItem {
                    name: name.clone(),
                    path: full_path.clone(),
                    is_dir: true,
                    size: sz,
                    item_type,
                    extension: String::new(),
                    category: cat,
                    importance: imp,
                    created_ms: *c_ms,
                    modified_ms: *m_ms,
                    accessed_ms: *a_ms,
                    analysis_explanation: explanation,
                };
                folder_items.push(item.clone());
                if indexed_items.len() < 10_000 {
                    indexed_items.push(item);
                }
            }
        }
    }

    folder_items.sort_by(|a, b| b.size.cmp(&a.size));
    // Deduplicate parent/child folders so Largest Folders displays distinct major directories first
    let mut largest_folders: Vec<StorageItem> = Vec::new();
    for f in &folder_items {
        let rel_depth = Path::new(&f.path)
            .strip_prefix(&root_path)
            .map(|r| r.components().count())
            .unwrap_or(99);
        if rel_depth <= 2 && largest_folders.len() < 12 {
            largest_folders.push(f.clone());
        }
    }
    if largest_folders.len() < 6 {
        for f in folder_items {
            if !largest_folders.iter().any(|existing| existing.path == f.path) {
                largest_folders.push(f);
                if largest_folders.len() >= 10 {
                    break;
                }
            }
        }
    }

    // Finalize Largest Files
    largest_files.sort_by(|a, b| b.size.cmp(&a.size));
    largest_files.truncate(50);

    // Generate Dynamic ASAO Storage Insights from actual discovered data
    let insights = generate_dynamic_insights(
        &target_drive,
        &category_bytes,
        &largest_folders,
        &largest_files,
        &indexed_items,
    );

    let mut discovered_extensions: Vec<String> = extension_set.into_iter().collect();
    discovered_extensions.sort();

    let initial_directory = read_directory_with_index(&target_drive.mount_point, &folder_sizes);

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let snapshot = StorageAnalysisSnapshot {
        selected_drive: target_drive.drive.clone(),
        drives,
        distribution,
        insights,
        largest_folders,
        largest_files,
        discovered_extensions,
        initial_directory,
        files_analyzed,
        folders_analyzed,
        bytes_analyzed,
        scan_timestamp_ms: now_ms,
    };

    let index = DriveStorageIndex {
        folder_sizes,
        indexed_items,
        snapshot: Some(snapshot.clone()),
    };

    Ok((index, snapshot))
}

fn generate_dynamic_insights(
    drive: &StorageDrive,
    category_bytes: &HashMap<StorageCategory, u64>,
    largest_folders: &[StorageItem],
    largest_files: &[StorageItem],
    indexed_items: &[StorageItem],
) -> Vec<StorageInsight> {
    let mut insights = Vec::new();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // 1. Check Old / Sizable Downloads
    let mut download_items: Vec<StorageInsightAffectedItem> = Vec::new();
    let mut download_bytes: u64 = 0;
    let mut downloads_folder_path: Option<String> = None;

    for item in indexed_items {
        let lp = item.path.to_lowercase();
        if lp.contains("\\downloads\\") && !item.is_dir && item.size >= 5 * 1024 * 1024 {
            let age_days = item
                .modified_ms
                .map(|m| now_ms.saturating_sub(m) / (1000 * 60 * 60 * 24))
                .unwrap_or(0);
            if age_days >= 7 || item.size >= 25 * 1024 * 1024 {
                download_bytes = download_bytes.saturating_add(item.size);
                if downloads_folder_path.is_none() {
                    if let Some(idx) = lp.find("\\downloads") {
                        downloads_folder_path = Some(item.path[..idx + 10].to_string());
                    }
                }
                if download_items.len() < 8 {
                    download_items.push(StorageInsightAffectedItem {
                        name: item.name.clone(),
                        path: item.path.clone(),
                        size: item.size,
                        is_dir: false,
                        category: item.category,
                        importance: item.importance,
                    });
                }
            }
        }
    }

    if download_bytes >= 20 * 1024 * 1024 {
        insights.push(StorageInsight {
            id: "insight-old-downloads".to_string(),
            insight_type: "old_downloads".to_string(),
            title: "Old downloads".to_string(),
            description:
                "Installers, archives, and media files accumulated in your Downloads directory."
                    .to_string(),
            size: download_bytes,
            affected_items: download_items,
            importance: StorageImportance::LOW,
            action: "review".to_string(),
            target_path: downloads_folder_path,
            target_category: Some(StorageCategory::USER),
        });
    }

    // 2. Check Application Cache
    let cache_total = category_bytes
        .get(&StorageCategory::CACHE)
        .copied()
        .unwrap_or(0);
    if cache_total >= 20 * 1024 * 1024 {
        let mut cache_affected: Vec<StorageInsightAffectedItem> = indexed_items
            .iter()
            .filter(|i| i.category == StorageCategory::CACHE && i.size >= 5 * 1024 * 1024)
            .take(8)
            .map(|i| StorageInsightAffectedItem {
                name: i.name.clone(),
                path: i.path.clone(),
                size: i.size,
                is_dir: i.is_dir,
                category: i.category,
                importance: i.importance,
            })
            .collect();
        cache_affected.sort_by(|a, b| b.size.cmp(&a.size));
        let target_p = cache_affected.first().map(|a| a.path.clone());

        insights.push(StorageInsight {
            id: "insight-app-cache".to_string(),
            insight_type: "application_caches".to_string(),
            title: "Application cache".to_string(),
            description: "Cached package stores, shader caches, and browser/runtime cache directories that can be safely regenerated.".to_string(),
            size: cache_total,
            affected_items: cache_affected,
            importance: StorageImportance::LOW,
            action: "review".to_string(),
            target_path: target_p,
            target_category: Some(StorageCategory::CACHE),
        });
    }

    // 3. Check Temporary Files
    let temp_total = category_bytes
        .get(&StorageCategory::TEMPORARY)
        .copied()
        .unwrap_or(0);
    if temp_total >= 10 * 1024 * 1024 {
        let mut temp_affected: Vec<StorageInsightAffectedItem> = indexed_items
            .iter()
            .filter(|i| i.category == StorageCategory::TEMPORARY && i.size >= 1024 * 1024)
            .take(8)
            .map(|i| StorageInsightAffectedItem {
                name: i.name.clone(),
                path: i.path.clone(),
                size: i.size,
                is_dir: i.is_dir,
                category: i.category,
                importance: i.importance,
            })
            .collect();
        temp_affected.sort_by(|a, b| b.size.cmp(&a.size));
        let target_p = temp_affected.first().map(|a| a.path.clone());

        insights.push(StorageInsight {
            id: "insight-temp-files".to_string(),
            insight_type: "large_temporary_files".to_string(),
            title: "Temporary files".to_string(),
            description: "Temporary OS staging files, crash dumps, and Recycle Bin contents occupying disk space.".to_string(),
            size: temp_total,
            affected_items: temp_affected,
            importance: StorageImportance::LOW,
            action: "review".to_string(),
            target_path: target_p,
            target_category: Some(StorageCategory::TEMPORARY),
        });
    }

    // 4. Check Large Standalone Files (Archives / Media / Binaries > 100 MB)
    let large_standalone: Vec<&StorageItem> = largest_files
        .iter()
        .filter(|f| {
            f.size >= 100 * 1024 * 1024
                && f.category != StorageCategory::SYSTEM
                && !f.path.to_lowercase().contains("\\downloads\\")
        })
        .take(6)
        .collect();

    if !large_standalone.is_empty() {
        let total_large: u64 = large_standalone.iter().map(|f| f.size).sum();
        let affected = large_standalone
            .iter()
            .map(|f| StorageInsightAffectedItem {
                name: f.name.clone(),
                path: f.path.clone(),
                size: f.size,
                is_dir: false,
                category: f.category,
                importance: f.importance,
            })
            .collect();

        insights.push(StorageInsight {
            id: "insight-large-files".to_string(),
            insight_type: "large_unused_files".to_string(),
            title: "Large standalone files".to_string(),
            description: "High-capacity archives, disk images, and media files consuming significant disk space.".to_string(),
            size: total_large,
            affected_items: affected,
            importance: StorageImportance::NORMAL,
            action: "review".to_string(),
            target_path: None,
            target_category: None,
        });
    }

    // 5. Check Drive Capacity Pressure if > 80% used
    if drive.usage_percentage >= 80.0 {
        let top_folder = largest_folders.first();
        insights.push(StorageInsight {
            id: "insight-capacity-pressure".to_string(),
            insight_type: "storage_critically_full".to_string(),
            title: format!("{} is {:.1}% full", drive.drive, drive.usage_percentage),
            description: "Free disk space is running low, which can impact virtual memory paging and system update staging.".to_string(),
            size: drive.used_capacity,
            affected_items: largest_folders
                .iter()
                .take(4)
                .map(|f| StorageInsightAffectedItem {
                    name: f.name.clone(),
                    path: f.path.clone(),
                    size: f.size,
                    is_dir: true,
                    category: f.category,
                    importance: f.importance,
                })
                .collect(),
            importance: StorageImportance::IMPORTANT,
            action: "review".to_string(),
            target_path: top_folder.map(|f| f.path.clone()),
            target_category: None,
        });
    }

    insights
}
