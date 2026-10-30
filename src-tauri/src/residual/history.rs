//! `InventorySnapshotStore`: a running record of every app ASAO has seen installed.
//!
//! Each entry keeps first-seen, last-seen and removed-on dates. An app that disappears
//! is marked removed and stays in the record, so a scan days later can still say
//! "recently uninstalled" even if ASAO was launched many times in between.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::inventory::InstalledApp;
use super::normalize::{normalize_name, normalize_publisher};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppHistoryEntry {
    pub key: String,
    pub display_name: String,
    pub publisher: Option<String>,
    /// Folders the app owned while installed (path keys), kept after it is removed.
    pub install_dirs: Vec<String>,
    pub package_family_name: Option<String>,
    /// "registry" or "store"
    pub source_class: String,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub removed_on_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryHistory {
    pub version: u32,
    pub entries: Vec<AppHistoryEntry>,
    pub last_updated_ms: u64,
    pub observations: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ObserveSummary {
    pub newly_seen: usize,
    pub newly_removed: usize,
    pub reinstalled: usize,
    /// Source classes whose removals were not recorded because that source looked broken.
    pub guarded_classes: Vec<String>,
}

/// Stable key for an app across versions: normalised name plus normalised publisher.
/// Store apps use their package family name. Apps without a display name have no key.
pub fn app_key(app: &InstalledApp) -> Option<String> {
    if let Some(family) = app.package_family_name.as_deref() {
        return Some(format!("store|{}", family.to_lowercase()));
    }
    let name = normalize_name(app.display_name.as_deref()?);
    if name.is_empty() {
        return None;
    }
    let publisher = app
        .publisher
        .as_deref()
        .map(normalize_publisher)
        .unwrap_or_default();
    Some(format!("{}|{}", name, publisher))
}

impl InventoryHistory {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<InventoryHistory>(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    /// Records what is installed right now.
    ///
    /// `failed_classes` lists sources that could not be read this time. Their apps are not
    /// marked removed. A source whose app count dropped by more than half since the last
    /// observation is treated the same way, so a broken read cannot erase the record.
    pub fn observe(
        &mut self,
        apps: &[InstalledApp],
        failed_classes: &[&str],
        now_ms: u64,
    ) -> ObserveSummary {
        let mut summary = ObserveSummary::default();

        // Group what is installed now by key, merging paths of duplicate entries.
        let mut current: HashMap<String, (String, Option<String>, Vec<String>, Option<String>, String)> =
            HashMap::new();
        for app in apps {
            let Some(key) = app_key(app) else { continue };
            let entry = current.entry(key).or_insert_with(|| {
                (
                    app.display_name.clone().unwrap_or_default(),
                    app.publisher.clone(),
                    Vec::new(),
                    app.package_family_name.clone(),
                    app.source.class().to_string(),
                )
            });
            for dir in app.owned_dirs() {
                if !entry.2.contains(&dir) {
                    entry.2.push(dir);
                }
            }
        }

        // Guard: skip removals for a source class that failed or shrank suspiciously.
        let mut guarded: Vec<String> = failed_classes.iter().map(|c| c.to_string()).collect();
        for class in ["registry", "store"] {
            let previously_present = self
                .entries
                .iter()
                .filter(|e| e.source_class == class && e.removed_on_ms.is_none())
                .count();
            let now_present = current.values().filter(|v| v.4 == class).count();
            if previously_present >= 4 && now_present * 2 < previously_present {
                guarded.push(class.to_string());
            }
        }
        guarded.sort();
        guarded.dedup();

        let mut seen_keys = std::collections::HashSet::new();
        for entry in &mut self.entries {
            if let Some((name, publisher, dirs, family, _)) = current.get(&entry.key) {
                seen_keys.insert(entry.key.clone());
                if entry.removed_on_ms.is_some() {
                    summary.reinstalled += 1;
                }
                entry.removed_on_ms = None;
                entry.last_seen_ms = now_ms;
                if !name.is_empty() {
                    entry.display_name = name.clone();
                }
                if publisher.is_some() {
                    entry.publisher = publisher.clone();
                }
                if family.is_some() {
                    entry.package_family_name = family.clone();
                }
                for dir in dirs {
                    if !entry.install_dirs.contains(dir) && entry.install_dirs.len() < 12 {
                        entry.install_dirs.push(dir.clone());
                    }
                }
            } else if entry.removed_on_ms.is_none() && !guarded.contains(&entry.source_class) {
                entry.removed_on_ms = Some(now_ms);
                summary.newly_removed += 1;
            }
        }

        for (key, (name, publisher, dirs, family, class)) in current {
            if seen_keys.contains(&key) {
                continue;
            }
            self.entries.push(AppHistoryEntry {
                key,
                display_name: name,
                publisher,
                install_dirs: dirs,
                package_family_name: family,
                source_class: class,
                first_seen_ms: now_ms,
                last_seen_ms: now_ms,
                removed_on_ms: None,
            });
            summary.newly_seen += 1;
        }

        self.version = 1;
        self.last_updated_ms = now_ms;
        self.observations += 1;
        summary.guarded_classes = guarded;
        summary
    }

    pub fn removed(&self) -> impl Iterator<Item = &AppHistoryEntry> {
        self.entries.iter().filter(|e| e.removed_on_ms.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::inventory::AppSource;

    fn app(name: &str, publisher: &str, loc: &str) -> InstalledApp {
        InstalledApp {
            id: name.to_lowercase(),
            display_name: Some(name.into()),
            publisher: Some(publisher.into()),
            install_location: Some(loc.into()),
            uninstall_string: None,
            display_icon: None,
            source: AppSource::RegistryMachine,
            hidden: false,
            package_family_name: None,
            package_name: None,
        }
    }

    fn base() -> Vec<InstalledApp> {
        vec![
            app("Alpha", "A Inc", r"C:\Program Files\Alpha"),
            app("Bravo", "B Inc", r"C:\Program Files\Bravo"),
            app("Charlie", "C Inc", r"C:\Program Files\Charlie"),
            app("Delta", "D Inc", r"C:\Program Files\Delta"),
            app("Echo", "E Inc", r"C:\Program Files\Echo"),
        ]
    }

    #[test]
    fn removal_survives_later_launches() {
        let mut h = InventoryHistory::default();
        h.observe(&base(), &[], 1);
        let mut without_bravo = base();
        without_bravo.retain(|a| a.display_name.as_deref() != Some("Bravo"));
        let s = h.observe(&without_bravo, &[], 2);
        assert_eq!(s.newly_removed, 1);
        // A third launch must keep the evidence, with the original removal date.
        h.observe(&without_bravo, &[], 3);
        let removed: Vec<_> = h.removed().collect();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].display_name, "Bravo");
        assert_eq!(removed[0].removed_on_ms, Some(2));
        assert_eq!(removed[0].install_dirs, vec![r"c:\program files\bravo".to_string()]);
    }

    #[test]
    fn reinstall_clears_removed_date_and_version_bumps_keep_key() {
        let mut h = InventoryHistory::default();
        h.observe(&base(), &[], 1);
        let mut v2 = base();
        v2[0] = app("Alpha 2.0 (x64)", "A Inc.", r"C:\Program Files\Alpha");
        let s = h.observe(&v2, &[], 2);
        assert_eq!(s.newly_removed, 0, "a version bump is not an uninstall");
        let mut gone = base();
        gone.remove(1);
        h.observe(&gone, &[], 3);
        let s = h.observe(&base(), &[], 4);
        assert_eq!(s.reinstalled, 1);
        assert_eq!(h.removed().count(), 0);
    }

    #[test]
    fn broken_read_does_not_mark_everything_removed() {
        let mut h = InventoryHistory::default();
        h.observe(&base(), &[], 1);
        let s = h.observe(&[], &[], 2);
        assert_eq!(s.newly_removed, 0);
        let s = h.observe(&[], &["registry"], 3);
        assert_eq!(s.newly_removed, 0);
        assert_eq!(h.removed().count(), 0);
    }
}
