//! Where the feature keeps its files, under ASAO's app data folder:
//!
//! ```text
//! <app data>\residual\app_history.json     every app seen, with first/last seen and removed-on dates
//! <app data>\residual\ignore_list.json     folders and apps the user chose to hide
//! <app data>\residual\logs\deletions.jsonl one line per deleted (or skipped) folder
//! <app data>\residual\logs\dry-run.jsonl   what a dry run would have deleted
//! <app data>\residual\logs\scan.log        access-denied folders and other scan notes
//! <app data>\residual\jobs\                hand-off files for the administrator helper
//! ```

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::normalize::path_key;

#[derive(Debug, Clone)]
pub struct ResidualFiles {
    pub dir: PathBuf,
}

impl ResidualFiles {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            dir: app_data_dir.join("residual"),
        }
    }

    pub fn history(&self) -> PathBuf {
        self.dir.join("app_history.json")
    }

    pub fn ignore_list(&self) -> PathBuf {
        self.dir.join("ignore_list.json")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.dir.join("logs")
    }

    pub fn deletion_log(&self) -> PathBuf {
        self.logs_dir().join("deletions.jsonl")
    }

    pub fn dry_run_log(&self) -> PathBuf {
        self.logs_dir().join("dry-run.jsonl")
    }

    pub fn scan_log(&self) -> PathBuf {
        self.logs_dir().join("scan.log")
    }

    pub fn jobs_dir(&self) -> PathBuf {
        self.dir.join("jobs")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredFolder {
    pub path: String,
    pub added_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredApp {
    pub key: String,
    pub name: String,
    pub added_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IgnoreList {
    pub folders: Vec<IgnoredFolder>,
    pub apps: Vec<IgnoredApp>,
    /// Apps the user does not want suggested on the Unused apps tab.
    #[serde(default)]
    pub unused_apps: Vec<IgnoredApp>,
}

impl IgnoreList {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn folder_paths(&self) -> Vec<String> {
        self.folders.iter().map(|f| f.path.clone()).collect()
    }

    pub fn app_keys(&self) -> Vec<String> {
        self.apps.iter().map(|a| a.key.clone()).collect()
    }

    pub fn add_folder(&mut self, path: &str, now_ms: u64) {
        let key = path_key(path);
        if !self.folders.iter().any(|f| path_key(&f.path) == key) {
            self.folders.push(IgnoredFolder {
                path: path.to_string(),
                added_ms: now_ms,
            });
        }
    }

    pub fn add_app(&mut self, key: &str, name: &str, now_ms: u64) {
        if !self.apps.iter().any(|a| a.key.eq_ignore_ascii_case(key)) {
            self.apps.push(IgnoredApp {
                key: key.to_string(),
                name: name.to_string(),
                added_ms: now_ms,
            });
        }
    }

    pub fn remove_folder(&mut self, path: &str) {
        let key = path_key(path);
        self.folders.retain(|f| path_key(&f.path) != key);
    }

    pub fn remove_app(&mut self, key: &str) {
        self.apps.retain(|a| !a.key.eq_ignore_ascii_case(key));
    }

    pub fn add_unused_app(&mut self, key: &str, name: &str, now_ms: u64) {
        if !self.unused_apps.iter().any(|a| a.key.eq_ignore_ascii_case(key)) {
            self.unused_apps.push(IgnoredApp {
                key: key.to_string(),
                name: name.to_string(),
                added_ms: now_ms,
            });
        }
    }

    pub fn remove_unused_app(&mut self, key: &str) {
        self.unused_apps.retain(|a| !a.key.eq_ignore_ascii_case(key));
    }

    pub fn unused_app_keys(&self) -> Vec<String> {
        self.unused_apps.iter().map(|a| a.key.clone()).collect()
    }
}

/// Appends one JSON object as a line. Logging must never break a scan or a delete.
pub fn append_jsonl<T: Serialize>(path: &Path, value: &T) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(line) = serde_json::to_string(value) {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{}", line);
        }
    }
}

/// Overwrites the scan log with the latest scan notes.
pub fn write_scan_log(path: &Path, lines: &[String]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, lines.join("\r\n"));
}
