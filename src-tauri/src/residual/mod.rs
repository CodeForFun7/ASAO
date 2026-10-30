//! Residual file cleanup: finds folders left behind by apps that are no longer installed.
//!
//! The pipeline is split so new sources (game launchers, Documents, Saved Games, registry)
//! can be added later as extra providers or scanners:
//!
//! - `inventory`   — `InventoryProvider`s for installed apps (registry, Microsoft Store)
//! - `history`     — `InventorySnapshotStore`: every app ever seen, with first/last seen and removed-on dates
//! - `references`  — executables referenced by services, scheduled tasks, Run keys and startup shortcuts
//! - `roots`       — `ResidualScanner` roots (known folders) and shallow, reparse-safe folder listing
//! - `exclusions`  — `ExclusionList`: folders that are never flagged, plus the user's ignore list
//! - `ownership`   — `OwnershipChecker`: is a folder owned by something still installed?
//! - `scoring`     — `ConfidenceScorer`: points table and High / Medium / Low thresholds
//! - `sizer`       — `FolderSizer`: size on disk and "last used", computed in the background
//! - `cleaner`     — `ResidualCleaner`: pre-delete checks, Recycle Bin delete, logging
//! - `elevated`    — runs the cleaner as administrator for Program Files and ProgramData items
//! - `engine`      — ties everything together for one scan or one pre-delete verification
//!
//! Core rule: a folder is a leftover only if it does NOT belong to any installed app.
//! When in doubt the folder is not flagged.

pub mod cleaner;
pub mod elevated;
pub mod engine;
pub mod exclusions;
pub mod history;
pub mod inventory;
pub mod normalize;
pub mod ownership;
pub mod references;
pub mod roots;
pub mod scoring;
pub mod sizer;
pub mod store_files;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const EVENT_RESIDUAL_SCAN_PROGRESS: &str = "RESIDUAL_SCAN_PROGRESS";
pub const EVENT_RESIDUAL_SIZE_UPDATE: &str = "RESIDUAL_SIZE_UPDATE";
pub const EVENT_RESIDUAL_SIZING_DONE: &str = "RESIDUAL_SIZING_DONE";

/// Default for "skip folders with any file modified in the last N days".
pub const DEFAULT_MIN_AGE_DAYS: u32 = 30;

/// Developer mode unlocks the 0-day age override. On in debug builds, or with `ASAO_DEV=1`.
pub fn dev_mode() -> bool {
    cfg!(debug_assertions)
        || std::env::var("ASAO_DEV")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

/// Clamp the age threshold. Zero is only allowed in developer mode.
pub fn sanitize_min_age_days(days: u32) -> u32 {
    let min = if dev_mode() { 0 } else { 1 };
    days.clamp(min, 365)
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RootKind {
    ProgramFiles,
    ProgramFilesX86,
    RoamingAppData,
    LocalAppData,
    LocalAppDataLow,
    ProgramData,
}

impl RootKind {
    pub fn is_program_files(&self) -> bool {
        matches!(self, RootKind::ProgramFiles | RootKind::ProgramFilesX86)
    }

    /// Machine-wide roots need administrator rights to delete from.
    pub fn needs_admin(&self) -> bool {
        matches!(
            self,
            RootKind::ProgramFiles | RootKind::ProgramFilesX86 | RootKind::ProgramData
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            RootKind::ProgramFiles => "Program Files",
            RootKind::ProgramFilesX86 => "Program Files (x86)",
            RootKind::RoamingAppData => "AppData\\Roaming",
            RootKind::LocalAppData => "AppData\\Local",
            RootKind::LocalAppDataLow => "AppData\\LocalLow",
            RootKind::ProgramData => "ProgramData",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanRoot {
    pub kind: RootKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResidualCategory {
    InstallRemnant,
    Cache,
    Logs,
    AppData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualCandidate {
    pub id: String,
    pub path: String,
    pub name: String,
    pub root_kind: RootKind,
    pub depth: u8,
    pub category: ResidualCategory,
    pub confidence: Confidence,
    pub score: i32,
    /// Plain-language scoring signals, e.g. "Name matches Notepad++ (+6)".
    pub reasons: Vec<String>,
    /// Key of the uninstalled app this folder was matched to, if any.
    pub matched_app_key: Option<String>,
    pub matched_app_name: Option<String>,
    pub removed_on_ms: Option<u64>,
    pub newest_modified_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub needs_admin: bool,
    pub in_onedrive: bool,
    pub preselected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualGroup {
    /// The uninstalled app's key, or "unknown" for pure orphans.
    pub key: String,
    pub app_name: String,
    pub is_unknown: bool,
    pub confidence: Confidence,
    pub removed_on_ms: Option<u64>,
    pub folders: Vec<ResidualCandidate>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualScanStats {
    pub roots_scanned: u32,
    pub folders_checked: u64,
    pub owned_skipped: u64,
    pub excluded_skipped: u64,
    pub recent_skipped: u64,
    pub access_denied: u64,
    pub installed_apps: usize,
    pub removed_apps_known: usize,
    pub references_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualScanResult {
    pub scan_id: u64,
    pub started_ms: u64,
    pub finished_ms: u64,
    pub min_age_days: u32,
    pub groups: Vec<ResidualGroup>,
    pub stats: ResidualScanStats,
    /// Lines worth showing to a developer (access denied, skipped roots, and so on).
    pub log: Vec<String>,
}

impl ResidualScanResult {
    pub fn candidates(&self) -> impl Iterator<Item = &ResidualCandidate> {
        self.groups.iter().flat_map(|g| g.folders.iter())
    }

    pub fn find_candidate(&self, path: &str) -> Option<&ResidualCandidate> {
        let wanted = normalize::path_key(path);
        self.candidates()
            .find(|c| normalize::path_key(&c.path) == wanted)
    }

    pub fn remove_paths(&mut self, paths: &[String]) {
        let keys: Vec<String> = paths.iter().map(|p| normalize::path_key(p)).collect();
        for g in &mut self.groups {
            g.folders
                .retain(|c| !keys.contains(&normalize::path_key(&c.path)));
        }
        self.groups.retain(|g| !g.folders.is_empty());
    }

    pub fn remove_app_group(&mut self, key: &str) {
        self.groups.retain(|g| g.is_unknown || g.key != key);
    }

    pub fn set_size(&mut self, path: &str, size: u64, newest: Option<u64>) {
        let wanted = normalize::path_key(path);
        for g in &mut self.groups {
            for c in &mut g.folders {
                if normalize::path_key(&c.path) == wanted {
                    c.size_bytes = Some(size);
                    if newest.is_some() {
                        c.newest_modified_ms = newest;
                    }
                }
            }
        }
    }

    /// Totals used by the Storage suggestion: High and Medium items with a known size.
    pub fn confident_summary(&self) -> LeftoverSummary {
        let mut apps = std::collections::HashSet::new();
        let mut bytes = 0u64;
        let mut folders = 0usize;
        for g in &self.groups {
            for c in &g.folders {
                if c.confidence >= Confidence::Medium {
                    folders += 1;
                    bytes = bytes.saturating_add(c.size_bytes.unwrap_or(0));
                    if !g.is_unknown {
                        apps.insert(g.key.clone());
                    }
                }
            }
        }
        LeftoverSummary {
            app_count: apps.len(),
            folder_count: folders,
            total_bytes: bytes,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverSummary {
    pub app_count: usize,
    pub folder_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualScanProgress {
    pub phase: String,
    pub current_path: String,
    pub folders_checked: u64,
    pub candidates_found: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualSizeUpdate {
    pub scan_id: u64,
    pub path: String,
    pub size_bytes: u64,
    pub newest_modified_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualSizingDone {
    pub scan_id: u64,
    pub total_bytes: u64,
}
