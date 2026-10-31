//! System tools used by the Nyra agent (the Python LangGraph service in `src-python`).
//!
//! The agent reaches these through the local tool bridge (`crate::bridge`), not through
//! `invoke`. They work on arbitrary paths chosen in conversation, so every one of them is
//! bounded (entry caps and time budgets) and every destructive one goes through
//! `protected_reason`, which refuses system folders, user profile roots and ASAO's own data.
//! The agent asks the user to confirm before calling any destructive one.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::analyzer::storage_analyzer::{classify_storage_item, StorageCategory, StorageImportance};
use crate::collector::storage::{normalize_win_path, system_time_to_ms};
use crate::commands::AppMonitoringState;

/// Progress lines for long-running agent tools. Payload: `AgentProgress`.
pub const EVENT_AGENT_PROGRESS: &str = "AGENT_TOOL_PROGRESS";

const ATTR_READONLY: u32 = 0x1;
const ATTR_HIDDEN: u32 = 0x2;
const ATTR_SYSTEM: u32 = 0x4;
const ATTR_REPARSE_POINT: u32 = 0x400;

const DAY_MS: u64 = 86_400_000;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProgress {
    pub operation_id: String,
    pub message: String,
    pub processed: u64,
}

/// Throttled progress emitter (at most one event every 400 ms).
struct Progress<'a> {
    app: &'a AppHandle,
    op: Option<String>,
    last: Instant,
}

impl<'a> Progress<'a> {
    fn new(app: &'a AppHandle, op: Option<String>) -> Self {
        let last = Instant::now()
            .checked_sub(Duration::from_secs(1))
            .unwrap_or_else(Instant::now);
        Self { app, op, last }
    }

    fn tick(&mut self, processed: u64, message: impl FnOnce() -> String) {
        if self.op.is_none() || self.last.elapsed() < Duration::from_millis(400) {
            return;
        }
        self.send(processed, message());
    }

    fn send(&mut self, processed: u64, message: String) {
        let Some(op) = &self.op else { return };
        self.last = Instant::now();
        let _ = self.app.emit(
            EVENT_AGENT_PROGRESS,
            AgentProgress { operation_id: op.clone(), message, processed },
        );
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Replaces `%NAME%` with environment variables and a leading `~` with the user profile.
fn expand_env(input: &str) -> String {
    let mut s = input.to_string();
    if s == "~" || s.starts_with("~\\") {
        if let Ok(home) = std::env::var("USERPROFILE") {
            s = format!("{}{}", home, &s[1..]);
        }
    }
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(value) => {
                        out.push_str(&rest[..start]);
                        out.push_str(&value);
                    }
                    Err(_) => out.push_str(&rest[..start + end + 2]),
                }
                rest = &after[end + 1..];
            }
            _ => break,
        }
    }
    out.push_str(rest);
    out
}

/// Turns a path typed in conversation into a full Windows path.
fn clean_input_path(raw: &str) -> Result<PathBuf, String> {
    let mut s = raw.trim().trim_matches('"').replace('/', "\\");
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    s = expand_env(&s);
    if s.len() == 2 && s.as_bytes()[1] == b':' {
        s.push('\\');
    }
    if s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    let p = PathBuf::from(&s);
    if !p.is_absolute() {
        return Err(format!(
            "'{}' is not a full path. Use a full path such as C:\\Users\\Name\\Downloads.",
            raw
        ));
    }
    Ok(p)
}

fn norm(p: &Path) -> String {
    normalize_win_path(p).trim_end_matches('\\').to_lowercase()
}

fn file_kind(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "heic" | "heif" | "tif" | "tiff"
        | "svg" | "ico" | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "psd" | "avif"
        | "jfif" => "image",
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" | "m4v" | "mpg" | "mpeg"
        | "3gp" | "m2ts" | "vob" => "video",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "opus" | "aiff" | "mid"
        | "midi" | "alac" => "audio",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "rtf" | "odt"
        | "ods" | "odp" | "csv" | "md" | "epub" | "pages" | "key" | "numbers" | "one" => {
            "document"
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "cab" | "tgz" => "archive",
        "iso" | "img" | "vhd" | "vhdx" | "vmdk" | "qcow2" | "wim" => "disk-image",
        "exe" | "msi" | "msix" | "msixbundle" | "appx" | "appxbundle" | "bat" | "cmd" | "ps1"
        | "com" | "scr" | "lnk" => "executable",
        "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "py" | "c" | "cpp" | "h" | "hpp" | "cs"
        | "go" | "java" | "kt" | "rb" | "php" | "swift" | "json" | "yaml" | "yml" | "toml"
        | "xml" | "html" | "css" | "scss" | "sql" | "sh" | "ipynb" | "lua" | "dart" => "code",
        "tmp" | "temp" | "bak" | "old" | "log" | "dmp" | "crdownload" | "part" | "partial"
        | "cache" | "etl" => "temporary",
        "ttf" | "otf" | "woff" | "woff2" => "font",
        "dll" | "sys" | "drv" | "ocx" | "mui" | "cat" | "pdb" | "lib" | "so" => "system",
        _ => "other",
    }
}

fn ext_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[cfg(windows)]
fn attrs_of(meta: &fs::Metadata) -> u32 {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes()
}

#[cfg(not(windows))]
fn attrs_of(_meta: &fs::Metadata) -> u32 {
    0
}

/// A file or folder as reported to the agent.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Files inside, for folders.
    pub item_count: Option<u64>,
    pub extension: String,
    pub kind: String,
    pub created_ms: Option<u64>,
    pub modified_ms: Option<u64>,
    pub accessed_ms: Option<u64>,
    pub hidden: bool,
}

struct WalkEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    size: u64,
    attrs: u32,
    created_ms: Option<u64>,
    modified_ms: Option<u64>,
    accessed_ms: Option<u64>,
    depth: u32,
}

impl WalkEntry {
    fn to_entry(&self) -> AgentFileEntry {
        let extension = if self.is_dir { String::new() } else { ext_of(&self.path) };
        let kind = if self.is_dir { "folder" } else { file_kind(&extension) };
        AgentFileEntry {
            path: normalize_win_path(&self.path),
            name: self.name.clone(),
            is_dir: self.is_dir,
            size: self.size,
            item_count: None,
            kind: kind.to_string(),
            extension,
            created_ms: self.created_ms,
            modified_ms: self.modified_ms,
            accessed_ms: self.accessed_ms,
            hidden: self.attrs & (ATTR_HIDDEN | ATTR_SYSTEM) != 0,
        }
    }
}

#[derive(PartialEq)]
enum Visit {
    Continue,
    SkipChildren,
}

#[derive(Default)]
struct WalkStats {
    scanned: u64,
    access_denied: u64,
    truncated: bool,
}

/// Depth-first walk that never follows links or junctions. Children of `root` have depth 1.
fn walk(
    root: &Path,
    max_depth: u32,
    max_entries: u64,
    deadline: Instant,
    mut visit: impl FnMut(&WalkEntry) -> Visit,
) -> WalkStats {
    let mut stats = WalkStats::default();
    let mut stack: Vec<(PathBuf, u32)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        let read = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                stats.access_denied += 1;
                continue;
            }
        };
        for entry in read.flatten() {
            if stats.scanned >= max_entries || Instant::now() >= deadline {
                stats.truncated = true;
                return stats;
            }
            stats.scanned += 1;
            let Ok(meta) = entry.metadata() else { continue };
            let attrs = attrs_of(&meta);
            let is_link = entry.file_type().map(|t| t.is_symlink()).unwrap_or(false)
                || attrs & ATTR_REPARSE_POINT != 0;
            let is_dir = meta.is_dir();
            let item = WalkEntry {
                path: entry.path(),
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir,
                size: if is_dir { 0 } else { meta.len() },
                attrs,
                created_ms: system_time_to_ms(meta.created()),
                modified_ms: system_time_to_ms(meta.modified()),
                accessed_ms: system_time_to_ms(meta.accessed()),
                depth: depth + 1,
            };
            match visit(&item) {
                Visit::SkipChildren => {}
                Visit::Continue => {
                    if is_dir && !is_link && depth + 1 < max_depth {
                        stack.push((item.path, depth + 1));
                    }
                }
            }
        }
    }
    stats
}

/// Total size and file count of a folder, bounded.
fn folder_size(path: &Path, max_entries: u64, budget: Duration) -> (u64, u64, bool) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    let stats = walk(path, u32::MAX, max_entries, Instant::now() + budget, |e| {
        if !e.is_dir {
            bytes = bytes.saturating_add(e.size);
            files += 1;
        }
        Visit::Continue
    });
    (bytes, files, stats.truncated)
}

// ---------------------------------------------------------------------------
// Protected paths
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn known_folder(id: &windows::core::GUID) -> Option<String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, HANDLE::default()).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownFolder {
    pub key: String,
    pub label: String,
    pub path: String,
    pub exists: bool,
}

fn known_folders() -> Vec<KnownFolder> {
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let mut list: Vec<(&str, &str, Option<String>)> = Vec::new();

    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::{
            FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music,
            FOLDERID_Pictures, FOLDERID_Videos,
        };
        list.push(("desktop", "Desktop", known_folder(&FOLDERID_Desktop)));
        list.push(("documents", "Documents", known_folder(&FOLDERID_Documents)));
        list.push(("downloads", "Downloads", known_folder(&FOLDERID_Downloads)));
        list.push(("pictures", "Pictures", known_folder(&FOLDERID_Pictures)));
        list.push(("music", "Music", known_folder(&FOLDERID_Music)));
        list.push(("videos", "Videos", known_folder(&FOLDERID_Videos)));
    }

    let fallback = |name: &str| format!("{}\\{}", home, name);
    let mut out: Vec<KnownFolder> = list
        .into_iter()
        .map(|(key, label, path)| {
            let path = path.unwrap_or_else(|| fallback(label));
            KnownFolder {
                key: key.into(),
                label: label.into(),
                exists: Path::new(&path).exists(),
                path,
            }
        })
        .collect();

    for (key, label, var) in [
        ("home", "User profile", "USERPROFILE"),
        ("appdata", "AppData (Roaming)", "APPDATA"),
        ("localappdata", "AppData (Local)", "LOCALAPPDATA"),
        ("temp", "Temporary files", "TEMP"),
        ("onedrive", "OneDrive", "OneDrive"),
        ("programdata", "ProgramData", "ProgramData"),
        ("programfiles", "Program Files", "ProgramFiles"),
        ("programfilesx86", "Program Files (x86)", "ProgramFiles(x86)"),
        ("windows", "Windows", "SystemRoot"),
    ] {
        if let Ok(path) = std::env::var(var) {
            out.push(KnownFolder {
                key: key.into(),
                label: label.into(),
                exists: Path::new(&path).exists(),
                path,
            });
        }
    }
    out
}

/// Why a path must not be deleted by the agent, or `None` if it may be.
fn protected_reason(app: &AppHandle, path: &Path) -> Option<String> {
    let p = norm(path);
    if p.len() <= 3 {
        return Some("Whole drives cannot be deleted.".into());
    }

    let env = |k: &str| std::env::var(k).ok().map(|v| norm(Path::new(&v)));

    for component in p.split('\\') {
        if component == "$recycle.bin" || component == "system volume information" {
            return Some("Windows manages this folder.".into());
        }
    }

    let mut trees: Vec<(String, &str)> = Vec::new();
    trees.push((
        env("SystemRoot").unwrap_or_else(|| "c:\\windows".into()),
        "It is part of Windows.",
    ));
    for k in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(v) = env(k) {
            trees.push((v, "Programs in Program Files must be removed with their uninstaller (use the unused apps or leftovers tools)."));
        }
    }
    if let Some(pd) = env("ProgramData") {
        trees.push((format!("{}\\microsoft", pd), "Windows and Microsoft Defender keep data here."));
    }
    let sys_drive = env("SystemDrive").unwrap_or_else(|| "c:".into());
    for name in ["recovery", "boot", "efi", "msocache"] {
        trees.push((format!("{}\\{}", sys_drive, name), "Windows needs it to start or recover."));
    }
    for (root, why) in &trees {
        if p == *root || p.starts_with(&format!("{}\\", root)) {
            return Some((*why).into());
        }
    }

    let mut exact: Vec<String> = Vec::new();
    for k in [
        "ProgramData", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "PUBLIC", "OneDrive",
        "OneDriveConsumer", "OneDriveCommercial", "TEMP", "TMP", "ALLUSERSPROFILE",
    ] {
        if let Some(v) = env(k) {
            exact.push(v);
        }
    }
    if let Some(local) = env("LOCALAPPDATA") {
        if let Some(parent) = Path::new(&local).parent() {
            exact.push(norm(&parent.join("LocalLow")));
            exact.push(norm(parent));
        }
    }
    for kf in known_folders() {
        exact.push(norm(Path::new(&kf.path)));
    }
    exact.push(format!("{}\\users", sys_drive));
    if exact.iter().any(|e| *e == p) {
        return Some("This is a main system or user folder. Its contents can be removed, but not the folder itself.".into());
    }

    if let Some(parent) = path.parent() {
        let parent_norm = norm(parent);
        if parent_norm == format!("{}\\users", sys_drive) {
            return Some("User profile folders cannot be deleted.".into());
        }
        if parent_norm.len() <= 3 {
            let name = p.rsplit('\\').next().unwrap_or("");
            if matches!(
                name,
                "pagefile.sys" | "hiberfil.sys" | "swapfile.sys" | "bootmgr" | "dumpstack.log.tmp"
                    | "bootnxt" | "users" | "program files" | "program files (x86)" | "programdata"
                    | "windows"
            ) {
                return Some("Windows needs this file or folder.".into());
            }
        }
    }

    if let Ok(dir) = app.path().app_data_dir() {
        let d = norm(&dir);
        if p == d || p.starts_with(&format!("{}\\", d)) {
            return Some("This is ASAO's own data folder.".into());
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let d = norm(dir);
            if p == d || p.starts_with(&format!("{}\\", d)) {
                return Some("ASAO is running from this folder.".into());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Known folders
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn agent_known_folders() -> Vec<KnownFolder> {
    known_folders()
}

// ---------------------------------------------------------------------------
// Path details
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathDetails {
    pub path: String,
    pub name: String,
    pub exists: bool,
    pub is_dir: bool,
    pub size: u64,
    pub file_count: Option<u64>,
    pub folder_count: Option<u64>,
    /// False when the folder was too large to measure completely.
    pub size_complete: bool,
    pub extension: String,
    pub kind: String,
    pub item_type: String,
    pub category: Option<StorageCategory>,
    pub importance: Option<StorageImportance>,
    pub explanation: String,
    pub created_ms: Option<u64>,
    pub modified_ms: Option<u64>,
    pub accessed_ms: Option<u64>,
    pub readonly: bool,
    pub hidden: bool,
    pub system: bool,
    pub parent: Option<String>,
    /// Set when the agent is not allowed to delete this path.
    pub protected_reason: Option<String>,
}

#[tauri::command]
pub async fn agent_path_details(app: AppHandle, path: String) -> Result<PathDetails, String> {
    let p = clean_input_path(&path)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path_str = normalize_win_path(&p);
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path_str.clone());
        let parent = p.parent().map(normalize_win_path);
        let protected = protected_reason(&app, &p);
        let Ok(meta) = fs::metadata(&p) else {
            return Ok(PathDetails {
                path: path_str,
                name,
                exists: false,
                is_dir: false,
                size: 0,
                file_count: None,
                folder_count: None,
                size_complete: true,
                extension: String::new(),
                kind: "missing".into(),
                item_type: String::new(),
                category: None,
                importance: None,
                explanation: "Nothing exists at this path.".into(),
                created_ms: None,
                modified_ms: None,
                accessed_ms: None,
                readonly: false,
                hidden: false,
                system: false,
                parent,
                protected_reason: protected,
            });
        };
        let is_dir = meta.is_dir();
        let attrs = attrs_of(&meta);
        let extension = if is_dir { String::new() } else { ext_of(&p) };
        let (size, file_count, folder_count, complete) = if is_dir {
            let mut bytes = 0u64;
            let mut files = 0u64;
            let mut folders = 0u64;
            let stats = walk(&p, u32::MAX, 400_000, Instant::now() + Duration::from_secs(25), |e| {
                if e.is_dir {
                    folders += 1;
                } else {
                    files += 1;
                    bytes = bytes.saturating_add(e.size);
                }
                Visit::Continue
            });
            (bytes, Some(files), Some(folders), !stats.truncated)
        } else {
            (meta.len(), None, None, true)
        };
        let modified_ms = system_time_to_ms(meta.modified());
        let (category, importance, item_type, explanation) =
            classify_storage_item(&path_str, &name, is_dir, &extension, size, modified_ms);
        Ok(PathDetails {
            kind: if is_dir { "folder".into() } else { file_kind(&extension).to_string() },
            path: path_str,
            name,
            exists: true,
            is_dir,
            size,
            file_count,
            folder_count,
            size_complete: complete,
            extension,
            item_type,
            category: Some(category),
            importance: Some(importance),
            explanation,
            created_ms: system_time_to_ms(meta.created()),
            modified_ms,
            accessed_ms: system_time_to_ms(meta.accessed()),
            readonly: attrs & ATTR_READONLY != 0,
            hidden: attrs & ATTR_HIDDEN != 0,
            system: attrs & ATTR_SYSTEM != 0,
            parent,
            protected_reason: protected,
        })
    })
    .await
    .map_err(|e| format!("Details thread error: {}", e))?
}

// ---------------------------------------------------------------------------
// Folder summary
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountBytes {
    pub key: String,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildStat {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub file_count: u64,
    pub modified_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderSummary {
    pub path: String,
    pub total_size: u64,
    pub file_count: u64,
    pub folder_count: u64,
    pub by_kind: Vec<CountBytes>,
    pub top_extensions: Vec<CountBytes>,
    pub largest_files: Vec<AgentFileEntry>,
    pub children: Vec<ChildStat>,
    pub child_count: u64,
    pub newest_modified_ms: Option<u64>,
    pub oldest_modified_ms: Option<u64>,
    /// Files not modified for a year.
    pub stale_files: u64,
    pub stale_bytes: u64,
    pub scanned_entries: u64,
    pub access_denied: u64,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

fn sorted_counts(map: HashMap<String, (u64, u64)>, take: usize) -> Vec<CountBytes> {
    let mut v: Vec<CountBytes> = map
        .into_iter()
        .map(|(key, (count, bytes))| CountBytes { key, count, bytes })
        .collect();
    v.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(b.count.cmp(&a.count)));
    v.truncate(take);
    v
}

#[tauri::command]
pub async fn agent_folder_summary(
    app: AppHandle,
    path: String,
    top: Option<u32>,
    operation_id: Option<String>,
) -> Result<FolderSummary, String> {
    let root = clean_input_path(&path)?;
    if !root.is_dir() {
        return Err(format!("'{}' is not a folder that exists.", normalize_win_path(&root)));
    }
    let top = top.unwrap_or(15).clamp(1, 100) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let mut progress = Progress::new(&app, operation_id);
        let mut total = 0u64;
        let mut files = 0u64;
        let mut folders = 0u64;
        let mut by_kind: HashMap<String, (u64, u64)> = HashMap::new();
        let mut by_ext: HashMap<String, (u64, u64)> = HashMap::new();
        let mut largest: Vec<AgentFileEntry> = Vec::new();
        let mut children: HashMap<String, ChildStat> = HashMap::new();
        let mut newest: Option<u64> = None;
        let mut oldest: Option<u64> = None;
        let stale_cut = now_ms().saturating_sub(365 * DAY_MS);
        let mut stale_files = 0u64;
        let mut stale_bytes = 0u64;
        let root_display = normalize_win_path(&root);

        let stats = walk(&root, u32::MAX, 2_000_000, Instant::now() + Duration::from_secs(90), |e| {
            // Attribute everything to the direct child of the root that contains it.
            let child_name = e
                .path
                .strip_prefix(&root)
                .ok()
                .and_then(|r| r.components().next())
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .unwrap_or_else(|| e.name.clone());
            if e.depth == 1 {
                children.entry(child_name.clone()).or_insert_with(|| ChildStat {
                    name: e.name.clone(),
                    path: normalize_win_path(&e.path),
                    is_dir: e.is_dir,
                    size: 0,
                    file_count: 0,
                    modified_ms: e.modified_ms,
                });
            }
            if e.is_dir {
                folders += 1;
            } else {
                files += 1;
                total = total.saturating_add(e.size);
                let ext = ext_of(&e.path);
                let kind = file_kind(&ext);
                let k = by_kind.entry(kind.to_string()).or_insert((0, 0));
                k.0 += 1;
                k.1 += e.size;
                let x = by_ext
                    .entry(if ext.is_empty() { "(none)".into() } else { ext })
                    .or_insert((0, 0));
                x.0 += 1;
                x.1 += e.size;
                if let Some(c) = children.get_mut(&child_name) {
                    c.size += e.size;
                    c.file_count += 1;
                }
                if let Some(m) = e.modified_ms {
                    newest = Some(newest.map_or(m, |n| n.max(m)));
                    oldest = Some(oldest.map_or(m, |o| o.min(m)));
                    if m < stale_cut {
                        stale_files += 1;
                        stale_bytes += e.size;
                    }
                }
                largest.push(e.to_entry());
                if largest.len() > top * 8 {
                    largest.sort_by(|a, b| b.size.cmp(&a.size));
                    largest.truncate(top);
                }
                progress.tick(files, || {
                    format!("Read {} files ({}) in {}", files, human_bytes(total), root_display)
                });
            }
            Visit::Continue
        });

        largest.sort_by(|a, b| b.size.cmp(&a.size));
        largest.truncate(top);
        let child_count = children.len() as u64;
        let mut children: Vec<ChildStat> = children.into_values().collect();
        children.sort_by(|a, b| b.size.cmp(&a.size));
        children.truncate(40);

        Ok(FolderSummary {
            path: root_display,
            total_size: total,
            file_count: files,
            folder_count: folders,
            by_kind: sorted_counts(by_kind, 20),
            top_extensions: sorted_counts(by_ext, 25),
            largest_files: largest,
            children,
            child_count,
            newest_modified_ms: newest,
            oldest_modified_ms: oldest,
            stale_files,
            stale_bytes,
            scanned_entries: stats.scanned,
            access_denied: stats.access_denied,
            truncated: stats.truncated,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    })
    .await
    .map_err(|e| format!("Folder summary thread error: {}", e))?
}

fn human_bytes(b: u64) -> String {
    let f = b as f64;
    if f >= 1024f64.powi(3) {
        format!("{:.2} GB", f / 1024f64.powi(3))
    } else if f >= 1024f64.powi(2) {
        format!("{:.1} MB", f / 1024f64.powi(2))
    } else if f >= 1024.0 {
        format!("{:.0} KB", f / 1024.0)
    } else {
        format!("{} B", b)
    }
}

// ---------------------------------------------------------------------------
// File search
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchFilesRequest {
    pub root: String,
    /// Any-of terms matched against names (case-insensitive, also ignoring spaces, `-`, `_` and `.`).
    pub name_terms: Vec<String>,
    /// Also match terms against the full path.
    pub match_full_path: bool,
    /// Extensions without the dot.
    pub extensions: Vec<String>,
    /// Kinds from `file_kind` (image, video, audio, document, ...).
    pub kinds: Vec<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub modified_before_ms: Option<u64>,
    pub modified_after_ms: Option<u64>,
    pub accessed_before_ms: Option<u64>,
    /// Return folders whose names match the terms (as one entry, not descended into).
    pub include_folders: bool,
    pub max_depth: Option<u32>,
    pub limit: Option<u32>,
    /// size | newest | oldest | name | least-accessed
    pub sort_by: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilesResult {
    pub root: String,
    pub matches: Vec<AgentFileEntry>,
    pub total_matches: u64,
    pub total_bytes: u64,
    pub scanned_entries: u64,
    pub access_denied: u64,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

fn squash(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[tauri::command]
pub async fn agent_search_files(
    app: AppHandle,
    request: SearchFilesRequest,
    operation_id: Option<String>,
) -> Result<SearchFilesResult, String> {
    let root = clean_input_path(&request.root)?;
    if !root.is_dir() {
        return Err(format!("'{}' is not a folder that exists.", normalize_win_path(&root)));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let mut progress = Progress::new(&app, operation_id);
        let terms: Vec<(String, String)> = request
            .name_terms
            .iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .map(|t| {
                let sq = squash(&t);
                (t, sq)
            })
            .collect();
        let exts: HashSet<String> = request
            .extensions
            .iter()
            .map(|e| e.trim().trim_start_matches('.').to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        let kinds: HashSet<String> = request.kinds.iter().map(|k| k.trim().to_lowercase()).collect();
        let limit = request.limit.unwrap_or(200).clamp(1, 1000) as usize;
        let max_depth = request.max_depth.unwrap_or(u32::MAX).max(1);

        let name_matches = |name: &str, path: &Path| -> bool {
            if terms.is_empty() {
                return true;
            }
            let lname = name.to_lowercase();
            let sname = squash(name);
            let hit = |hay: &str, squashed: &str| {
                terms.iter().any(|(t, sq)| hay.contains(t.as_str()) || (sq.len() >= 3 && squashed.contains(sq.as_str())))
            };
            if hit(&lname, &sname) {
                return true;
            }
            if request.match_full_path {
                let lp = normalize_win_path(path).to_lowercase();
                return hit(&lp, &squash(&lp));
            }
            false
        };

        let mut matches: Vec<AgentFileEntry> = Vec::new();
        let mut total_matches = 0u64;
        let mut total_bytes = 0u64;
        let root_display = normalize_win_path(&root);
        let mut scanned_local = 0u64;

        let stats = walk(&root, max_depth, 3_000_000, Instant::now() + Duration::from_secs(120), |e| {
            scanned_local += 1;
            progress.tick(scanned_local, || {
                format!("Checked {} items in {}, {} matches so far", scanned_local, root_display, total_matches)
            });
            if e.is_dir {
                let folder_hit = request.include_folders
                    && !terms.is_empty()
                    && {
                        let lname = e.name.to_lowercase();
                        let sname = squash(&e.name);
                        terms.iter().any(|(t, sq)| lname.contains(t.as_str()) || (sq.len() >= 3 && sname.contains(sq.as_str())))
                    };
                if folder_hit {
                    let (bytes, count, _) = folder_size(&e.path, 200_000, Duration::from_secs(10));
                    let mut entry = e.to_entry();
                    entry.size = bytes;
                    entry.item_count = Some(count);
                    total_matches += 1;
                    total_bytes += bytes;
                    if matches.len() < 20_000 {
                        matches.push(entry);
                    }
                    return Visit::SkipChildren;
                }
                return Visit::Continue;
            }
            let ext = ext_of(&e.path);
            if !exts.is_empty() && !exts.contains(&ext) {
                return Visit::Continue;
            }
            if !kinds.is_empty() && !kinds.contains(file_kind(&ext)) {
                return Visit::Continue;
            }
            if request.min_size.is_some_and(|m| e.size < m) || request.max_size.is_some_and(|m| e.size > m) {
                return Visit::Continue;
            }
            if let Some(cut) = request.modified_before_ms {
                if e.modified_ms.map_or(true, |m| m >= cut) {
                    return Visit::Continue;
                }
            }
            if let Some(cut) = request.modified_after_ms {
                if e.modified_ms.map_or(true, |m| m < cut) {
                    return Visit::Continue;
                }
            }
            if let Some(cut) = request.accessed_before_ms {
                // Windows may not update access times; fall back to the modified time.
                let last = e.accessed_ms.max(e.modified_ms);
                if last.map_or(true, |m| m >= cut) {
                    return Visit::Continue;
                }
            }
            if !name_matches(&e.name, &e.path) {
                return Visit::Continue;
            }
            total_matches += 1;
            total_bytes += e.size;
            if matches.len() < 20_000 {
                matches.push(e.to_entry());
            }
            Visit::Continue
        });

        match request.sort_by.as_deref().unwrap_or("size") {
            "newest" => matches.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms)),
            "oldest" => matches.sort_by(|a, b| a.modified_ms.cmp(&b.modified_ms)),
            "name" => matches.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            "least-accessed" => matches.sort_by(|a, b| {
                a.accessed_ms.max(a.modified_ms).cmp(&b.accessed_ms.max(b.modified_ms))
            }),
            _ => matches.sort_by(|a, b| b.size.cmp(&a.size)),
        }
        matches.truncate(limit);

        Ok(SearchFilesResult {
            root: root_display,
            matches,
            total_matches,
            total_bytes,
            scanned_entries: stats.scanned,
            access_denied: stats.access_denied,
            truncated: stats.truncated,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    })
    .await
    .map_err(|e| format!("Search thread error: {}", e))?
}

// ---------------------------------------------------------------------------
// Duplicate files
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DuplicateRequest {
    pub root: String,
    pub min_size: Option<u64>,
    pub extensions: Vec<String>,
    pub kinds: Vec<String>,
    pub max_groups: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: u64,
    pub count: u64,
    pub wasted_bytes: u64,
    /// Oldest first: the first file is usually the original.
    pub files: Vec<AgentFileEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateResult {
    pub root: String,
    pub groups: Vec<DuplicateGroup>,
    pub total_groups: u64,
    pub duplicate_files: u64,
    pub reclaimable_bytes: u64,
    pub files_scanned: u64,
    pub bytes_hashed: u64,
    pub truncated: bool,
    pub elapsed_ms: u64,
}

const PARTIAL_HASH_BYTES: u64 = 64 * 1024;

fn hash_file(path: &Path, limit: Option<u64>, hashed: &mut u64) -> Option<[u8; 32]> {
    let mut f = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut remaining = limit.unwrap_or(u64::MAX);
    while remaining > 0 {
        let want = (buf.len() as u64).min(remaining) as usize;
        let n = f.read(&mut buf[..want]).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        *hashed += n as u64;
        remaining -= n as u64;
    }
    Some(hasher.finalize().into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[tauri::command]
pub async fn agent_find_duplicates(
    app: AppHandle,
    request: DuplicateRequest,
    operation_id: Option<String>,
) -> Result<DuplicateResult, String> {
    let root = clean_input_path(&request.root)?;
    if !root.is_dir() {
        return Err(format!("'{}' is not a folder that exists.", normalize_win_path(&root)));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let deadline = Instant::now() + Duration::from_secs(240);
        let mut progress = Progress::new(&app, operation_id);
        let min_size = request.min_size.unwrap_or(1024 * 1024).max(1);
        let exts: HashSet<String> = request
            .extensions
            .iter()
            .map(|e| e.trim().trim_start_matches('.').to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        let kinds: HashSet<String> = request.kinds.iter().map(|k| k.trim().to_lowercase()).collect();
        let root_display = normalize_win_path(&root);

        // 1. Group candidate files by size.
        let mut by_size: HashMap<u64, Vec<AgentFileEntry>> = HashMap::new();
        let mut files_scanned = 0u64;
        let stats = walk(&root, u32::MAX, 3_000_000, deadline, |e| {
            if e.is_dir || e.size < min_size || e.attrs & ATTR_SYSTEM != 0 {
                return Visit::Continue;
            }
            let ext = ext_of(&e.path);
            if !exts.is_empty() && !exts.contains(&ext) {
                return Visit::Continue;
            }
            if !kinds.is_empty() && !kinds.contains(file_kind(&ext)) {
                return Visit::Continue;
            }
            files_scanned += 1;
            progress.tick(files_scanned, || format!("Listed {} files in {}", files_scanned, root_display));
            by_size.entry(e.size).or_default().push(e.to_entry());
            Visit::Continue
        });
        let mut truncated = stats.truncated;

        let mut sizes: Vec<u64> = by_size.iter().filter(|(_, v)| v.len() > 1).map(|(s, _)| *s).collect();
        sizes.sort_unstable_by(|a, b| b.cmp(a));
        let candidates: u64 = sizes.iter().map(|s| by_size[s].len() as u64).sum();
        progress.send(0, format!("{} files share a size with another file. Comparing contents...", candidates));

        // 2. Compare first 64 KB, then the whole file.
        let mut groups: Vec<DuplicateGroup> = Vec::new();
        let mut hashed = 0u64;
        let mut compared = 0u64;
        for size in sizes {
            if Instant::now() >= deadline {
                truncated = true;
                break;
            }
            let files = by_size.remove(&size).unwrap_or_default();
            let mut partial: HashMap<[u8; 32], Vec<AgentFileEntry>> = HashMap::new();
            for f in files {
                compared += 1;
                if let Some(h) = hash_file(Path::new(&f.path), Some(PARTIAL_HASH_BYTES), &mut hashed) {
                    partial.entry(h).or_default().push(f);
                }
            }
            for (ph, same_start) in partial {
                if same_start.len() < 2 {
                    continue;
                }
                let mut full: HashMap<[u8; 32], Vec<AgentFileEntry>> = HashMap::new();
                if size <= PARTIAL_HASH_BYTES {
                    full.insert(ph, same_start);
                } else {
                    for f in same_start {
                        if Instant::now() >= deadline {
                            truncated = true;
                            break;
                        }
                        if let Some(h) = hash_file(Path::new(&f.path), None, &mut hashed) {
                            full.entry(h).or_default().push(f);
                        }
                    }
                }
                for (h, mut same) in full {
                    if same.len() < 2 {
                        continue;
                    }
                    same.sort_by(|a, b| {
                        a.created_ms.unwrap_or(u64::MAX).cmp(&b.created_ms.unwrap_or(u64::MAX))
                    });
                    let count = same.len() as u64;
                    groups.push(DuplicateGroup {
                        hash: hex(&h[..8]),
                        size,
                        count,
                        wasted_bytes: size * (count - 1),
                        files: same,
                    });
                }
            }
            progress.tick(compared, || {
                format!(
                    "Compared {} of {} files, {} duplicate groups found",
                    compared,
                    candidates,
                    groups.len()
                )
            });
        }

        groups.sort_by(|a, b| b.wasted_bytes.cmp(&a.wasted_bytes));
        let total_groups = groups.len() as u64;
        let duplicate_files = groups.iter().map(|g| g.count - 1).sum();
        let reclaimable_bytes = groups.iter().map(|g| g.wasted_bytes).sum();
        groups.truncate(request.max_groups.unwrap_or(50).clamp(1, 500) as usize);

        Ok(DuplicateResult {
            root: root_display,
            groups,
            total_groups,
            duplicate_files,
            reclaimable_bytes,
            files_scanned,
            bytes_hashed: hashed,
            truncated,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    })
    .await
    .map_err(|e| format!("Duplicate search thread error: {}", e))?
}

// ---------------------------------------------------------------------------
// Temporary files
// ---------------------------------------------------------------------------

struct TempDef {
    id: String,
    label: String,
    description: &'static str,
    paths: Vec<PathBuf>,
    safety: &'static str,
    needs_admin: bool,
}

fn browser_profiles(user_data: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = fs::read_dir(user_data) else { return Vec::new() };
    read.flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (name == "Default" || name.starts_with("Profile ")).then(|| (name, e.path()))
        })
        .collect()
}

fn temp_defs() -> Vec<TempDef> {
    let env = |k: &str| std::env::var(k).ok().map(PathBuf::from);
    let mut defs = Vec::new();
    if let Some(t) = env("TEMP") {
        defs.push(TempDef {
            id: "user-temp".into(),
            label: "Your temporary files".into(),
            description: "Files apps create while they run and usually forget to remove.",
            paths: vec![t],
            safety: "safe",
            needs_admin: false,
        });
    }
    let windows = env("SystemRoot").unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    defs.push(TempDef {
        id: "windows-temp".into(),
        label: "Windows temporary files".into(),
        description: "Temporary files from Windows and installers.",
        paths: vec![windows.join("Temp")],
        safety: "safe",
        needs_admin: true,
    });
    defs.push(TempDef {
        id: "windows-update-cache".into(),
        label: "Windows Update downloads".into(),
        description: "Update files already installed. Windows downloads them again if needed.",
        paths: vec![windows.join("SoftwareDistribution").join("Download")],
        safety: "review",
        needs_admin: true,
    });
    if let Some(local) = env("LOCALAPPDATA") {
        defs.push(TempDef {
            id: "crash-dumps".into(),
            label: "Crash dumps and error reports".into(),
            description: "Memory dumps and reports saved when apps crashed.",
            paths: vec![
                local.join("CrashDumps"),
                local.join("Microsoft").join("Windows").join("WER"),
            ],
            safety: "safe",
            needs_admin: false,
        });
        defs.push(TempDef {
            id: "internet-cache".into(),
            label: "Windows internet cache".into(),
            description: "Cached web content used by Windows components.",
            paths: vec![local.join("Microsoft").join("Windows").join("INetCache")],
            safety: "safe",
            needs_admin: false,
        });
        for (id, label, user_data) in [
            ("chrome-cache", "Google Chrome cache", local.join("Google").join("Chrome").join("User Data")),
            ("edge-cache", "Microsoft Edge cache", local.join("Microsoft").join("Edge").join("User Data")),
            ("brave-cache", "Brave cache", local.join("BraveSoftware").join("Brave-Browser").join("User Data")),
        ] {
            for (profile, dir) in browser_profiles(&user_data) {
                defs.push(TempDef {
                    id: format!("{}-{}", id, profile.to_lowercase().replace(' ', "-")),
                    label: format!("{} ({})", label, profile),
                    description: "Cached pages and images. Sign-ins, history and bookmarks are not touched. Close the browser first for the best result.",
                    paths: vec![dir.join("Cache"), dir.join("Code Cache"), dir.join("GPUCache")],
                    safety: "safe",
                    needs_admin: false,
                });
            }
        }
        if let Ok(read) = fs::read_dir(local.join("Mozilla").join("Firefox").join("Profiles")) {
            for e in read.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                defs.push(TempDef {
                    id: format!("firefox-cache-{}", name.to_lowercase()),
                    label: format!("Firefox cache ({})", name),
                    description: "Cached pages and images. Close Firefox first.",
                    paths: vec![e.path().join("cache2")],
                    safety: "safe",
                    needs_admin: false,
                });
            }
        }
        defs.push(TempDef {
            id: "developer-caches".into(),
            label: "Developer package caches (npm, pip, Yarn)".into(),
            description: "Downloaded packages kept for reuse. Tools download them again when needed.",
            paths: vec![
                local.join("npm-cache"),
                local.join("pip").join("Cache"),
                local.join("Yarn").join("Cache"),
            ],
            safety: "review",
            needs_admin: false,
        });
    }
    defs
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempLocation {
    pub id: String,
    pub label: String,
    pub description: String,
    pub paths: Vec<String>,
    pub exists: bool,
    pub size_bytes: u64,
    pub file_count: u64,
    /// "safe" or "review"
    pub safety: String,
    pub needs_admin: bool,
    pub truncated: bool,
}

#[cfg(windows)]
fn recycle_bin_info() -> Option<(u64, u64)> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info).ok()? };
    Some((info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64))
}

#[cfg(not(windows))]
fn recycle_bin_info() -> Option<(u64, u64)> {
    None
}

#[tauri::command]
pub async fn agent_temp_locations(app: AppHandle, operation_id: Option<String>) -> Result<Vec<TempLocation>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let defs = temp_defs();
        let mut progress = Progress::new(&app, operation_id);
        progress.send(0, format!("Measuring {} temporary file locations", defs.len()));
        let mut out: Vec<TempLocation> = std::thread::scope(|s| {
            let handles: Vec<_> = defs
                .iter()
                .map(|d| {
                    s.spawn(move || {
                        let mut size = 0u64;
                        let mut files = 0u64;
                        let mut truncated = false;
                        let mut exists = false;
                        for p in &d.paths {
                            if p.is_dir() {
                                exists = true;
                                let (b, c, t) = folder_size(p, 500_000, Duration::from_secs(30));
                                size += b;
                                files += c;
                                truncated |= t;
                            }
                        }
                        TempLocation {
                            id: d.id.clone(),
                            label: d.label.clone(),
                            description: d.description.to_string(),
                            paths: d.paths.iter().map(|p| normalize_win_path(p)).collect(),
                            exists,
                            size_bytes: size,
                            file_count: files,
                            safety: d.safety.to_string(),
                            needs_admin: d.needs_admin,
                            truncated,
                        }
                    })
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        if let Some((size, items)) = recycle_bin_info() {
            out.push(TempLocation {
                id: "recycle-bin".into(),
                label: "Recycle Bin".into(),
                description: "Files you already deleted. Emptying it removes them for good.".into(),
                paths: Vec::new(),
                exists: true,
                size_bytes: size,
                file_count: items,
                safety: "review".into(),
                needs_admin: false,
                truncated: false,
            });
        }
        out.retain(|l| l.exists);
        out.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        Ok(out)
    })
    .await
    .map_err(|e| format!("Temporary files thread error: {}", e))?
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempCleanLocation {
    pub id: String,
    pub label: String,
    pub freed_bytes: u64,
    pub deleted_files: u64,
    pub skipped_in_use: u64,
    pub skipped_recent: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempCleanReport {
    pub freed_bytes: u64,
    pub deleted_files: u64,
    pub skipped_in_use: u64,
    pub skipped_recent: u64,
    pub locations: Vec<TempCleanLocation>,
    pub unknown_ids: Vec<String>,
}

/// Permanently deletes the contents of the chosen temporary locations. The location folders
/// themselves stay. Files changed within `min_age_hours` and files in use are skipped.
#[tauri::command]
pub async fn agent_clean_temp(
    app: AppHandle,
    location_ids: Vec<String>,
    min_age_hours: Option<u32>,
    operation_id: Option<String>,
) -> Result<TempCleanReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let defs = temp_defs();
        let mut progress = Progress::new(&app, operation_id);
        let cut = now_ms().saturating_sub(min_age_hours.unwrap_or(24) as u64 * 3_600_000);
        let known: HashSet<&str> = defs.iter().map(|d| d.id.as_str()).collect();
        let unknown_ids: Vec<String> = location_ids
            .iter()
            .filter(|id| !known.contains(id.as_str()) && id.as_str() != "recycle-bin")
            .cloned()
            .collect();
        let mut report = TempCleanReport {
            freed_bytes: 0,
            deleted_files: 0,
            skipped_in_use: 0,
            skipped_recent: 0,
            locations: Vec::new(),
            unknown_ids,
        };
        for def in defs.iter().filter(|d| location_ids.contains(&d.id)) {
            let mut loc = TempCleanLocation {
                id: def.id.clone(),
                label: def.label.clone(),
                freed_bytes: 0,
                deleted_files: 0,
                skipped_in_use: 0,
                skipped_recent: 0,
            };
            for root in &def.paths {
                if !root.is_dir() {
                    continue;
                }
                let mut dirs: Vec<PathBuf> = Vec::new();
                walk(root, u32::MAX, 1_000_000, Instant::now() + Duration::from_secs(120), |e| {
                    if e.attrs & ATTR_REPARSE_POINT != 0 {
                        return Visit::SkipChildren;
                    }
                    if e.is_dir {
                        dirs.push(e.path.clone());
                        return Visit::Continue;
                    }
                    let recent = e.modified_ms.map_or(true, |m| m > cut);
                    if recent {
                        loc.skipped_recent += 1;
                        return Visit::Continue;
                    }
                    match fs::remove_file(&e.path) {
                        Ok(()) => {
                            loc.deleted_files += 1;
                            loc.freed_bytes += e.size;
                        }
                        Err(_) => loc.skipped_in_use += 1,
                    }
                    progress.tick(loc.deleted_files, || {
                        format!("{}: removed {} files ({})", def.label, loc.deleted_files, human_bytes(loc.freed_bytes))
                    });
                    Visit::Continue
                });
                // Remove emptied folders, deepest first. Non-empty ones fail and stay.
                dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
                for d in dirs {
                    let _ = fs::remove_dir(&d);
                }
            }
            report.freed_bytes += loc.freed_bytes;
            report.deleted_files += loc.deleted_files;
            report.skipped_in_use += loc.skipped_in_use;
            report.skipped_recent += loc.skipped_recent;
            report.locations.push(loc);
        }
        if report.deleted_files > 0 {
            let places: Vec<&str> = report.locations.iter().map(|l| l.label.as_str()).collect();
            crate::audit::record(
                "files",
                &format!("Deleted {} temporary file(s)", report.deleted_files),
                &format!("{} freed from {}", crate::audit::human_bytes(report.freed_bytes), places.join(", ")),
                None,
                Some("Temporary files were deleted permanently. ASAO cannot bring them back."),
                None,
            );
        }
        Ok(report)
    })
    .await
    .map_err(|e| format!("Cleanup thread error: {}", e))?
}

#[tauri::command]
pub async fn agent_empty_recycle_bin() -> Result<u64, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let before = recycle_bin_info().map(|(s, _)| s).unwrap_or(0);
        let items = recycle_bin_info().map(|(_, n)| n).unwrap_or(0);
        if items == 0 {
            return Ok(0);
        }
        #[cfg(windows)]
        unsafe {
            use windows::core::PCWSTR;
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::Shell::{
                SHEmptyRecycleBinW, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND,
            };
            SHEmptyRecycleBinW(
                HWND::default(),
                PCWSTR::null(),
                SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
            )
            .map_err(|e| format!("Windows could not empty the Recycle Bin: {}", e))?;
        }
        crate::audit::record(
            "recycle-bin",
            &format!("Emptied the Recycle Bin ({} item(s))", items),
            &format!("Freed {}", crate::audit::human_bytes(before)),
            None,
            Some("The Recycle Bin was emptied. ASAO cannot bring the items back."),
            None,
        );
        Ok(before)
    })
    .await
    .map_err(|e| format!("Recycle Bin thread error: {}", e))?
}

// ---------------------------------------------------------------------------
// Guarded deletion
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteOutcome {
    pub path: String,
    /// deleted | skipped | failed
    pub status: String,
    pub reason: Option<String>,
    pub size_bytes: u64,
    pub is_dir: bool,
    pub recycled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteReport {
    pub permanent: bool,
    pub freed_bytes: u64,
    pub deleted: u64,
    pub skipped: u64,
    pub failed: u64,
    pub outcomes: Vec<DeleteOutcome>,
}

/// Deletes files or folders, to the Recycle Bin unless `permanent`. Protected paths are refused.
#[tauri::command]
pub async fn agent_delete_paths(
    app: AppHandle,
    paths: Vec<String>,
    permanent: bool,
    operation_id: Option<String>,
) -> Result<DeleteReport, String> {
    if paths.is_empty() {
        return Err("No paths were given.".into());
    }
    if paths.len() > 5000 {
        return Err("Too many paths in one request (limit 5000).".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut progress = Progress::new(&app, operation_id);
        let mut report = DeleteReport {
            permanent,
            freed_bytes: 0,
            deleted: 0,
            skipped: 0,
            failed: 0,
            outcomes: Vec::new(),
        };

        let mut cleaned: Vec<(String, Result<PathBuf, String>)> =
            paths.iter().map(|raw| (raw.clone(), clean_input_path(raw))).collect();
        // Folders before their contents, so contained paths can be reported as included.
        cleaned.sort_by_key(|(_, p)| p.as_ref().map(|p| p.components().count()).unwrap_or(0));
        let mut removed_dirs: Vec<String> = Vec::new();
        let total = cleaned.len();

        for (index, (raw, parsed)) in cleaned.into_iter().enumerate() {
            let mut outcome = DeleteOutcome {
                path: raw.clone(),
                status: "skipped".into(),
                reason: None,
                size_bytes: 0,
                is_dir: false,
                recycled: false,
            };
            let path = match parsed {
                Ok(p) => p,
                Err(e) => {
                    outcome.reason = Some(e);
                    report.skipped += 1;
                    report.outcomes.push(outcome);
                    continue;
                }
            };
            outcome.path = normalize_win_path(&path);
            let key = norm(&path);
            if removed_dirs.iter().any(|d| key.starts_with(&format!("{}\\", d))) {
                outcome.status = "deleted".into();
                outcome.reason = Some("Removed together with its parent folder.".into());
                report.outcomes.push(outcome);
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                outcome.reason = Some("It no longer exists.".into());
                report.skipped += 1;
                report.outcomes.push(outcome);
                continue;
            };
            if let Some(why) = protected_reason(&app, &path) {
                outcome.reason = Some(format!("Protected: {}", why));
                report.skipped += 1;
                report.outcomes.push(outcome);
                continue;
            }
            outcome.is_dir = meta.is_dir();
            outcome.size_bytes = if outcome.is_dir {
                folder_size(&path, 300_000, Duration::from_secs(15)).0
            } else {
                meta.len()
            };
            progress.send(index as u64, format!("Removing {} ({}/{})", outcome.path, index + 1, total));
            match crate::residual::cleaner::delete_folder(&path, permanent) {
                Ok(()) => {
                    outcome.status = "deleted".into();
                    outcome.recycled = !permanent;
                    report.deleted += 1;
                    report.freed_bytes += outcome.size_bytes;
                    if outcome.is_dir {
                        removed_dirs.push(key);
                    }
                }
                Err(e) => {
                    outcome.status = "failed".into();
                    outcome.reason = Some(e.replace("The folder is still there", "It is still there"));
                    report.failed += 1;
                }
            }
            report.outcomes.push(outcome);
        }
        // Items removed with their parent folder are covered by the parent's entry.
        for o in report.outcomes.iter().filter(|o| o.status == "deleted" && o.reason.is_none()) {
            let label = if o.is_dir { "folder" } else { "file" };
            crate::audit::record_deletion("files", label, &o.path, Some(o.size_bytes), o.recycled);
        }
        Ok(report)
    })
    .await
    .map_err(|e| format!("Delete thread error: {}", e))?
}

// ---------------------------------------------------------------------------
// Process termination
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminateOutcome {
    pub pid: u32,
    pub name: String,
    /// terminated | refused | not-found | failed
    pub status: String,
    pub message: String,
}

const NEVER_TERMINATE: &[&str] = &[
    "system", "registry", "secure system", "memory compression", "smss.exe", "csrss.exe",
    "wininit.exe", "winlogon.exe", "services.exe", "lsass.exe", "lsaiso.exe", "svchost.exe",
    "dwm.exe", "fontdrvhost.exe", "msmpeng.exe", "ntoskrnl.exe", "audiodg.exe",
];

#[cfg(windows)]
fn terminate_pid(pid: u32) -> Result<bool, String> {
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid)
            .map_err(|e| format!("Windows did not allow ASAO to end it ({}). It may need administrator rights.", e))?;
        let result = TerminateProcess(handle, 1);
        let exited = result.is_ok() && WaitForSingleObject(handle, 3000) == WAIT_OBJECT_0;
        let _ = CloseHandle(handle);
        result.map_err(|e| format!("Windows could not end it: {}", e))?;
        Ok(exited)
    }
}

#[cfg(not(windows))]
fn terminate_pid(_pid: u32) -> Result<bool, String> {
    Err("Not supported".into())
}

/// Ends one process. `expected_name` guards against the PID having been reused.
#[tauri::command]
pub async fn agent_terminate_process(
    state: State<'_, AppMonitoringState>,
    pid: u32,
    expected_name: Option<String>,
) -> Result<TerminateOutcome, String> {
    let snapshot = {
        let mut engine = state
            .engine
            .lock()
            .map_err(|_| "Failed to acquire process analyzer lock".to_string())?;
        engine.get_latest_or_collect()?
    };
    let refuse = |name: &str, message: &str| TerminateOutcome {
        pid,
        name: name.to_string(),
        status: "refused".into(),
        message: message.to_string(),
    };
    let Some(proc_info) = snapshot.processes.iter().find(|p| p.pid == pid) else {
        return Ok(TerminateOutcome {
            pid,
            name: expected_name.unwrap_or_default(),
            status: "not-found".into(),
            message: "No running process has this ID. It may have closed already.".into(),
        });
    };
    let name = proc_info.name.clone();
    if pid <= 4 || pid == std::process::id() {
        return Ok(refuse(&name, "This process cannot be ended."));
    }
    if let Some(expected) = expected_name.as_deref() {
        let a = expected.trim().to_lowercase();
        let b = name.to_lowercase();
        if !a.is_empty() && a != b && a.trim_end_matches(".exe") != b.trim_end_matches(".exe") {
            return Ok(refuse(&name, &format!("This ID now belongs to {}, not {}. Nothing was ended.", name, expected)));
        }
    }
    if proc_info.is_system_critical || NEVER_TERMINATE.contains(&name.to_lowercase().as_str()) {
        return Ok(refuse(&name, "This is a Windows process. Ending it could crash or destabilise the PC."));
    }
    let result = tauri::async_runtime::spawn_blocking(move || terminate_pid(pid))
        .await
        .map_err(|e| format!("Terminate thread error: {}", e))?;
    Ok(match result {
        Ok(exited) => {
            crate::audit::record(
                "processes",
                &format!("Ended process \"{}\"", name),
                &format!("Process ID {}", pid),
                None,
                Some("An ended process cannot be restored. Start the program again yourself."),
                None,
            );
            TerminateOutcome {
            pid,
            name,
            status: "terminated".into(),
            message: if exited {
                "The process ended.".into()
            } else {
                "Windows accepted the request; the process is shutting down.".into()
            },
        }
        }
        Err(message) => TerminateOutcome { pid, name, status: "failed".into(), message },
    })
}
