//! `OwnershipChecker`: decides whether a folder still belongs to something installed.
//!
//! A folder is OWNED, and is never flagged, when any of these holds:
//! 1. Its name matches an installed app's name or publisher (normalised, fuzzy for
//!    names over 4 characters, also with the publisher stripped from the product name),
//!    or an installed app's name contains the folder's name (similar-name check).
//! 2. An installed app's install, uninstaller or icon folder is the folder, is inside it,
//!    or contains it.
//! 3. It matches an installed Store package.
//! 4. It holds an executable that is running or referenced by a startup entry,
//!    service or scheduled task.
//! 5. It is excluded (see `exclusions`).  — checked by the engine
//! 6. A file inside was modified recently. — checked by the engine (needs a folder walk)

use super::inventory::InstalledApp;
use super::normalize::{
    folder_name_variants, is_same_or_inside, match_names, normalize_name, normalize_publisher,
    path_key, strip_publisher, token_contains, NameMatch,
};
use super::ScanRoot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedReason {
    /// Folder name matches an installed app's publisher (a vendor folder).
    Publisher(String),
    Name(String),
    SimilarName(String),
    Path(String),
    StorePackage(String),
    Executable(String),
}

impl OwnedReason {
    pub fn describe(&self) -> String {
        match self {
            OwnedReason::Publisher(p) => format!("Publisher folder of installed apps from {}", p),
            OwnedReason::Name(a) => format!("Belongs to installed app {}", a),
            OwnedReason::SimilarName(a) => format!("Name is part of installed app {}", a),
            OwnedReason::Path(a) => format!("Install folder of {}", a),
            OwnedReason::StorePackage(p) => format!("Data of installed Store app {}", p),
            OwnedReason::Executable(e) => format!("Contains {}, which is running or starts automatically", e),
        }
    }
}

#[derive(Debug, Clone)]
struct IndexedApp {
    display_name: String,
    name_norm: String,
    stripped_norm: Option<String>,
    publisher_raw: Option<String>,
    publisher_norm: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct OwnershipChecker {
    apps: Vec<IndexedApp>,
    /// (path key, app display name)
    owned_dirs: Vec<(String, String)>,
    store_families: Vec<(String, String)>,
    store_names: Vec<(String, String)>,
    exe_paths: Vec<String>,
}

impl OwnershipChecker {
    /// `exe_paths`: path keys of running and auto-started executables.
    pub fn build(installed: &[InstalledApp], exe_paths: &[String], roots: &[ScanRoot]) -> Self {
        let root_keys: Vec<String> = roots
            .iter()
            .map(|r| path_key(&r.path.to_string_lossy()))
            .collect();
        let windir = std::env::var("WINDIR").map(|w| path_key(&w)).unwrap_or_default();

        let mut checker = OwnershipChecker::default();
        for app in installed {
            let label = app
                .display_name
                .clone()
                .or_else(|| app.package_name.clone())
                .unwrap_or_else(|| app.id.clone());

            if let Some(name) = app.display_name.as_deref() {
                let name_norm = normalize_name(name);
                if !name_norm.is_empty() {
                    let publisher_norm = app
                        .publisher
                        .as_deref()
                        .map(normalize_publisher)
                        .filter(|p| !p.is_empty());
                    let stripped_norm = publisher_norm
                        .as_deref()
                        .and_then(|p| strip_publisher(&name_norm, p));
                    checker.apps.push(IndexedApp {
                        display_name: name.to_string(),
                        name_norm,
                        stripped_norm,
                        publisher_raw: app.publisher.clone(),
                        publisher_norm,
                    });
                }
            }

            for dir in app.owned_dirs() {
                // A path that is a root (or holds one) would make every folder look owned.
                // Paths under Windows cannot hold anything we scan.
                let too_broad = dir.len() <= 3
                    || root_keys.iter().any(|r| is_same_or_inside(r, &dir))
                    || (!windir.is_empty() && is_same_or_inside(&dir, &windir));
                if !too_broad {
                    checker.owned_dirs.push((dir, label.clone()));
                }
            }

            if let Some(family) = app.package_family_name.as_deref() {
                checker
                    .store_families
                    .push((family.to_lowercase(), label.clone()));
            }
            if let Some(pkg) = app.package_name.as_deref() {
                checker.store_names.push((pkg.to_lowercase(), label.clone()));
            }
        }
        checker.exe_paths = exe_paths.to_vec();
        checker
    }

    /// Is `folder_norm` the publisher of something installed? Used for vendor folders.
    pub fn installed_publisher(&self, folder_norm: &str) -> Option<String> {
        if folder_norm.is_empty() {
            return None;
        }
        self.apps.iter().find_map(|a| {
            let p = a.publisher_norm.as_deref()?;
            if match_names(folder_norm, p) == NameMatch::Exact {
                Some(a.publisher_raw.clone().unwrap_or_else(|| p.to_string()))
            } else {
                None
            }
        })
    }

    /// Rules 1 to 4. Returns why the folder is owned, or `None` if nothing claims it.
    pub fn owner_of(&self, folder_path: &str, folder_name: &str) -> Option<OwnedReason> {
        let key = path_key(folder_path);
        let norm = normalize_name(folder_name);
        // "staruml-updater" and "io.testfully.app" are also checked as "staruml" and "testfully".
        let mut names = vec![norm.clone()];
        names.extend(folder_name_variants(folder_name));
        names.retain(|n| !n.is_empty());

        // Rule 1: name matches first. A folder named after an installed app is that app's
        // folder, even when the publisher has the same name ("Discord" by Discord Inc.).
        for n in &names {
            for app in &self.apps {
                let candidates = [Some(app.name_norm.as_str()), app.stripped_norm.as_deref()];
                for c in candidates.into_iter().flatten() {
                    if match_names(n, c) != NameMatch::None {
                        return Some(OwnedReason::Name(app.display_name.clone()));
                    }
                }
            }
        }
        // Then publisher matches: a vendor folder ("Mozilla") whose children are evaluated.
        if let Some(p) = self.installed_publisher(&norm) {
            return Some(OwnedReason::Publisher(p));
        }
        if !norm.is_empty() {
            for app in &self.apps {
                if let Some(p) = app.publisher_norm.as_deref() {
                    if match_names(&norm, p) == NameMatch::Fuzzy {
                        return Some(OwnedReason::Publisher(
                            app.publisher_raw.clone().unwrap_or_else(|| p.to_string()),
                        ));
                    }
                }
            }
            // Similar-name check: an installed app whose name contains this folder's name
            for n in &names {
                for app in &self.apps {
                    if token_contains(&app.name_norm, n) {
                        return Some(OwnedReason::SimilarName(app.display_name.clone()));
                    }
                }
            }
        }

        // Rule 2: install, uninstaller or icon folders
        for (dir, label) in &self.owned_dirs {
            if is_same_or_inside(&key, dir) || is_same_or_inside(dir, &key) {
                return Some(OwnedReason::Path(label.clone()));
            }
        }

        // Rule 3: Store packages (LocalAppData\Packages\<family name>)
        let lower = folder_name.to_lowercase();
        for (family, label) in &self.store_families {
            if &lower == family {
                return Some(OwnedReason::StorePackage(label.clone()));
            }
        }
        for (name, label) in &self.store_names {
            if &lower == name || lower.starts_with(&format!("{}_", name)) {
                return Some(OwnedReason::StorePackage(label.clone()));
            }
        }

        // Rule 4: running or auto-started executables inside
        for exe in &self.exe_paths {
            if is_same_or_inside(exe, &key) {
                let file = exe.rsplit('\\').next().unwrap_or(exe).to_string();
                return Some(OwnedReason::Executable(file));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::inventory::AppSource;
    use crate::residual::RootKind;
    use std::path::PathBuf;

    fn app(name: &str, publisher: &str, loc: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: name.to_lowercase(),
            display_name: Some(name.into()),
            publisher: Some(publisher.into()),
            install_location: loc.map(|s| s.to_string()),
            uninstall_string: None,
            display_icon: None,
            source: AppSource::RegistryMachine,
            hidden: false,
            package_family_name: None,
            package_name: None,
        }
    }

    fn roots() -> Vec<ScanRoot> {
        vec![
            ScanRoot { kind: RootKind::ProgramFiles, path: PathBuf::from(r"C:\Program Files") },
            ScanRoot { kind: RootKind::RoamingAppData, path: PathBuf::from(r"C:\Users\x\AppData\Roaming") },
        ]
    }

    fn checker() -> OwnershipChecker {
        let mut store = app("Microsoft.WindowsTerminal", "Microsoft Corporation", None);
        store.package_family_name = Some("Microsoft.WindowsTerminal_8wekyb3d8bbwe".into());
        store.package_name = Some("Microsoft.WindowsTerminal".into());
        store.source = AppSource::Store;
        let mut hidden = app("Hidden Runtime", "Contoso", None);
        hidden.display_name = None;
        hidden.hidden = true;
        hidden.install_location = Some(r"C:\Program Files\HiddenRuntime".into());
        OwnershipChecker::build(
            &[
                app("Mozilla Firefox (x64 en-US)", "Mozilla", Some(r"C:\Program Files\Mozilla Firefox")),
                app("Chrome Remote Desktop Host", "Google LLC", None),
                app("Notepad++ (64-bit x64)", "Notepad++ Team", None),
                store,
                hidden,
                // An app whose icon sits directly in a root must not claim the whole root.
                InstalledApp {
                    display_icon: Some(r"C:\Program Files\icon.ico".into()),
                    ..app("Loose Icon App", "Loose", None)
                },
            ],
            &[r"c:\users\x\appdata\roaming\runner\runner.exe".into()],
            &roots(),
        )
    }

    #[test]
    fn name_publisher_and_stripped_matches() {
        let c = checker();
        assert!(matches!(c.owner_of(r"C:\Users\x\AppData\Roaming\Mozilla", "Mozilla"), Some(OwnedReason::Publisher(_))));
        assert!(matches!(c.owner_of(r"C:\Users\x\AppData\Roaming\Firefox", "Firefox"), Some(OwnedReason::Name(_))));
        assert!(matches!(c.owner_of(r"C:\Users\x\AppData\Roaming\Notepad++", "Notepad++"), Some(OwnedReason::Name(_))));
        // Fuzzy: one letter off
        assert!(c.owner_of(r"C:\Users\x\AppData\Roaming\Notepad+", "Notepad+").is_some());
    }

    #[test]
    fn similar_name_check_protects_folder() {
        let c = checker();
        assert!(matches!(c.owner_of(r"C:\Users\x\AppData\Roaming\Chrome", "Chrome"), Some(OwnedReason::SimilarName(_))));
    }

    #[test]
    fn path_store_and_executable_rules() {
        let c = checker();
        assert!(matches!(c.owner_of(r"C:\Program Files\Mozilla Firefox", "Mozilla Firefox"), Some(_)));
        assert!(matches!(c.owner_of(r"C:\Program Files\HiddenRuntime", "HiddenRuntime"), Some(OwnedReason::Path(_))));
        assert!(matches!(
            c.owner_of(r"C:\Users\x\AppData\Local\Packages\Microsoft.WindowsTerminal_8wekyb3d8bbwe", "Microsoft.WindowsTerminal_8wekyb3d8bbwe"),
            Some(_)
        ));
        assert!(matches!(c.owner_of(r"C:\Users\x\AppData\Roaming\Runner", "Runner"), Some(OwnedReason::Executable(_))));
        // A folder nobody claims
        assert_eq!(c.owner_of(r"C:\Program Files\Audacity", "Audacity"), None);
    }
}
