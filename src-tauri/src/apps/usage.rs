//! When was each program last used?
//!
//! Three sources, none perfect on its own:
//! - UserAssist (see `residual::evidence::userassist_records`): programs launched from Explorer
//!   or Start. Misses programs started by other programs or at login.
//! - Prefetch: Windows' own record of every program start. Only readable as administrator.
//! - ASAO's monitor: every program ASAO sees running, recorded from now on.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::UsageSource;
use crate::residual::evidence::{is_installer_like, RunRecord};
use crate::residual::normalize::{is_same_or_inside, path_key};
use crate::residual::roots::system_time_ms;

/// Programs ASAO has seen running, by path key, with the last time seen.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStore {
    pub last_seen: HashMap<String, u64>,
    /// The program path as written on disk, by path key (for display).
    #[serde(default)]
    pub paths: HashMap<String, String>,
    /// When tracking started on this PC.
    pub since_ms: u64,
}

impl UsageStore {
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
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    /// Records the programs running now. Returns true when anything changed.
    pub fn record<'a>(&mut self, running: impl Iterator<Item = &'a str>, now_ms: u64) -> bool {
        if self.since_ms == 0 {
            self.since_ms = now_ms;
        }
        let mut changed = false;
        for exe in running {
            let key = path_key(exe);
            if key.len() < 4 {
                continue;
            }
            self.paths.entry(key.clone()).or_insert_with(|| exe.to_string());
            let entry = self.last_seen.entry(key).or_insert(0);
            if now_ms > *entry {
                *entry = now_ms;
                changed = true;
            }
        }
        // Keep the file bounded: drop the oldest entries past 20,000.
        if self.last_seen.len() > 20_000 {
            let mut all: Vec<(String, u64)> = self.last_seen.drain().collect();
            all.sort_by(|a, b| b.1.cmp(&a.1));
            all.truncate(15_000);
            self.last_seen = all.into_iter().collect();
            let keep: std::collections::HashSet<String> = self.last_seen.keys().cloned().collect();
            self.paths.retain(|k, _| keep.contains(k));
        }
        changed
    }
}

/// Last run per program file name ("chrome.exe"), from Windows Prefetch.
/// `None` when the folder cannot be read (ASAO is not running as administrator).
pub fn prefetch_last_runs() -> Option<HashMap<String, u64>> {
    let windir = std::env::var("WINDIR").ok()?;
    let rd = fs::read_dir(Path::new(&windir).join("Prefetch")).ok()?;
    let mut out: HashMap<String, u64> = HashMap::new();
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if let Some(exe) = prefetch_exe_name(&name) {
            if let Some(ms) = entry.metadata().ok().and_then(|m| system_time_ms(m.modified())) {
                let e = out.entry(exe).or_insert(0);
                if ms > *e {
                    *e = ms;
                }
            }
        }
    }
    Some(out)
}

/// "CHROME.EXE-1A2B3C4D.pf" gives "chrome.exe".
pub fn prefetch_exe_name(file_name: &str) -> Option<String> {
    let lower = file_name.to_lowercase();
    let stem = lower.strip_suffix(".pf")?;
    let (exe, hash) = stem.rsplit_once('-')?;
    if hash.len() == 8 && hash.chars().all(|c| c.is_ascii_hexdigit()) && exe.ends_with(".exe") {
        Some(exe.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_running_programs() {
        let mut u = UsageStore::default();
        assert!(u.record([r"C:\Apps\Tool\tool.exe"].into_iter(), 10));
        assert_eq!(u.since_ms, 10);
        assert!(!u.record([r"c:\apps\tool\tool.exe"].into_iter(), 5)); // older time: no change
        assert!(u.record([r"C:\Apps\Tool\tool.exe"].into_iter(), 20));
        assert_eq!(u.last_seen.get(r"c:\apps\tool\tool.exe"), Some(&20));
        assert_eq!(u.since_ms, 10);
    }

    #[test]
    fn prefetch_names() {
        assert_eq!(prefetch_exe_name("CHROME.EXE-1A2B3C4D.pf").as_deref(), Some("chrome.exe"));
        assert_eq!(prefetch_exe_name("MY-TOOL.EXE-0000ABCD.pf").as_deref(), Some("my-tool.exe"));
        assert_eq!(prefetch_exe_name("Layout.ini"), None);
        assert_eq!(prefetch_exe_name("AgGlFaultHistory.db"), None);
    }
}

/// Last use of anything inside these folders (path keys), or of a Store app's launch id,
/// from the launch record, ASAO's monitor and Prefetch. The newest wins.
pub fn last_use_in(
    runs: &[RunRecord],
    usage: &UsageStore,
    prefetch: Option<&HashMap<String, u64>>,
    list_exes: &dyn Fn(&str) -> Vec<String>,
    dirs: &[String],
    store_family: Option<&str>,
) -> Option<(u64, UsageSource)> {
    let mut best: Option<(u64, UsageSource)> = None;
    let mut bump = |t: Option<u64>, src: UsageSource| {
        if let Some(t) = t {
            if t > 0 && best.map(|b| t > b.0).unwrap_or(true) {
                best = Some((t, src));
            }
        }
    };
    let inside = |key: &str| dirs.iter().any(|d| is_same_or_inside(key, d));
    let family_prefix = store_family.map(|f| format!("{}!", f.to_lowercase()));
    for r in runs {
        if r.is_program {
            if inside(&path_key(&r.target)) {
                bump(r.last_run_ms, UsageSource::LaunchRecord);
            }
        } else if let Some(prefix) = &family_prefix {
            if r.target.to_lowercase().starts_with(prefix.as_str()) {
                bump(r.last_run_ms, UsageSource::LaunchRecord);
            }
        }
    }
    for (exe, t) in &usage.last_seen {
        if inside(exe) {
            bump(Some(*t), UsageSource::AsaoMonitor);
        }
    }
    if let Some(prefetch) = prefetch {
        for d in dirs.iter().take(3) {
            for name in list_exes(d).into_iter().take(60) {
                let stem = name.trim_end_matches(".exe");
                if is_installer_like(stem) {
                    continue;
                }
                bump(prefetch.get(&name).copied(), UsageSource::Prefetch);
            }
        }
    }
    best
}
