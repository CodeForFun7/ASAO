//! `ExclusionList`: folders that are never flagged, at any confidence.
//!
//! Names are compared case-insensitively against the folder's own name. Extend the
//! constants below to protect more shared vendor, runtime or launcher folders.

use std::collections::HashSet;

use super::normalize::{is_same_or_inside, path_key};

/// Never flagged, and never looked inside.
pub const NEVER_FLAG_NAMES: &[&str] = &[
    // Windows and Microsoft shared folders
    "microsoft", "windows", "common files", "windowsapps", "modifiablewindowsapps",
    "package cache", "installer", "windows defender", "windows defender advanced threat protection",
    "windows mail", "windows media player", "windows multimedia platform", "windows nt",
    "windows photo viewer", "windows portable devices", "windows security", "windows sidebar",
    "windowspowershell", "internet explorer", "msbuild", "reference assemblies", "windows kits",
    "microsoft.net", "microsoft office", "microsoft shared", "uninstall information",
    "comms", "connecteddevicesplatform", "peerdistrepub", "publishers", "virtualstore",
    "placeholdertilelogofolder", "history", "temporary internet files", "application data",
    "usoshared", "usoprivate", "ssh", "regid.1991-06.com.microsoft", "softwaredistribution",
    "windowsholographicdevices", "desktop", "documents", "start menu", "templates", "favorites",
    // Drivers and hardware vendors
    "nvidia corporation", "nvidia", "amd", "ati technologies", "intel", "realtek",
    // Runtimes and package managers
    ".net", "dotnet", "nuget", "npm", "npm-cache", "pip", "python", "java", "chocolatey",
    // Temporary and system caches
    "temp", "tmp", "crashdumps", "d3dscache", "squirreltemp",
    // Game launchers (their libraries are out of scope for this version)
    "steam", "steamapps", "epic games", "epicgameslauncher", "unrealengine", "battle.net",
    "blizzard entertainment", "ea", "ea games", "ea desktop", "electronic arts", "origin",
    "origin games", "ubisoft", "ubisoft game launcher", "gog.com", "gog galaxy", "riot games",
    // ASAO itself
    "asao", "com.asao.app",
];

/// Never flagged themselves, but their direct children are evaluated one by one.
/// `%LOCALAPPDATA%\Programs` holds per-user installs; `Packages` holds Store app data.
pub const CHILDREN_ONLY_NAMES: &[&str] = &["programs", "packages"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExclusionDecision {
    Allowed,
    /// Evaluate the children instead of the folder itself.
    ChildrenOnly,
    Never(String),
}

#[derive(Debug, Clone, Default)]
pub struct ExclusionList {
    never_names: HashSet<String>,
    children_only_names: HashSet<String>,
    /// Path keys that are protected together with everything inside them.
    protected_paths: Vec<String>,
    /// Path keys the user chose to ignore.
    ignored_folders: HashSet<String>,
    /// App keys the user chose to ignore.
    ignored_apps: HashSet<String>,
}

impl ExclusionList {
    pub fn new(protected_paths: Vec<String>) -> Self {
        let mut list = Self {
            never_names: NEVER_FLAG_NAMES.iter().map(|s| s.to_string()).collect(),
            children_only_names: CHILDREN_ONLY_NAMES.iter().map(|s| s.to_string()).collect(),
            protected_paths: Vec::new(),
            ignored_folders: HashSet::new(),
            ignored_apps: HashSet::new(),
        };
        for p in protected_paths {
            list.add_protected_path(&p);
        }
        // Windows itself and the drive roots are always protected.
        if let Ok(windir) = std::env::var("WINDIR").or_else(|_| std::env::var("SystemRoot")) {
            list.add_protected_path(&windir);
        }
        list
    }

    pub fn add_protected_path(&mut self, p: &str) {
        let key = path_key(p);
        if key.len() > 3 && !self.protected_paths.contains(&key) {
            self.protected_paths.push(key);
        }
    }

    pub fn with_ignores(mut self, folders: &[String], apps: &[String]) -> Self {
        self.ignored_folders = folders.iter().map(|p| path_key(p)).collect();
        self.ignored_apps = apps.iter().map(|a| a.to_lowercase()).collect();
        self
    }

    pub fn is_ignored_app(&self, app_key: &str) -> bool {
        self.ignored_apps.contains(&app_key.to_lowercase())
    }

    pub fn check(&self, path: &str, name: &str) -> ExclusionDecision {
        let key = path_key(path);
        let lower = name.trim().to_lowercase();

        // Drive roots
        if key.len() <= 3 {
            return ExclusionDecision::Never("Drive root".into());
        }
        if self
            .protected_paths
            .iter()
            .any(|p| is_same_or_inside(&key, p) || is_same_or_inside(p, &key))
        {
            return ExclusionDecision::Never("Protected system or ASAO folder".into());
        }
        if self.ignored_folders.contains(&key) {
            return ExclusionDecision::Never("On your ignore list".into());
        }
        if self.never_names.contains(&lower)
            || lower.starts_with("microsoft.")
            || lower.starts_with("microsoft ")
            || lower.starts_with("windows ")
        {
            return ExclusionDecision::Never(format!("{} is a shared system or vendor folder", name));
        }
        if self.children_only_names.contains(&lower) {
            return ExclusionDecision::ChildrenOnly;
        }
        ExclusionDecision::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_folders_are_never_flagged() {
        let ex = ExclusionList::new(vec![]);
        assert!(matches!(ex.check(r"C:\ProgramData\NVIDIA Corporation", "NVIDIA Corporation"), ExclusionDecision::Never(_)));
        assert!(matches!(ex.check(r"C:\Users\x\AppData\Local\Microsoft", "Microsoft"), ExclusionDecision::Never(_)));
        assert!(matches!(ex.check(r"C:\Program Files\Steam", "steam"), ExclusionDecision::Never(_)));
        assert!(matches!(ex.check(r"C:\Users\x\AppData\Local\Microsoft.Edge", "Microsoft.Edge"), ExclusionDecision::Never(_)));
        assert!(matches!(ex.check(r"C:\", "C:"), ExclusionDecision::Never(_)));
    }

    #[test]
    fn programs_and_packages_are_children_only() {
        let ex = ExclusionList::new(vec![]);
        assert_eq!(ex.check(r"C:\Users\x\AppData\Local\Programs", "Programs"), ExclusionDecision::ChildrenOnly);
        assert_eq!(ex.check(r"C:\Users\x\AppData\Local\Packages", "Packages"), ExclusionDecision::ChildrenOnly);
        assert_eq!(ex.check(r"C:\Users\x\AppData\Roaming\Audacity", "Audacity"), ExclusionDecision::Allowed);
    }

    #[test]
    fn protected_paths_and_ignore_list() {
        let ex = ExclusionList::new(vec![r"C:\Users\x\AppData\Roaming\com.asao.app".into()])
            .with_ignores(&[r"C:\Users\x\AppData\Roaming\KeepMe".into()], &["foo|bar".into()]);
        assert!(matches!(ex.check(r"C:\Users\x\AppData\Roaming\com.asao.app", "com.asao.app"), ExclusionDecision::Never(_)));
        assert!(matches!(ex.check(r"c:\users\x\appdata\roaming\keepme\", "KeepMe"), ExclusionDecision::Never(_)));
        assert!(ex.is_ignored_app("FOO|BAR"));
        if let Ok(windir) = std::env::var("WINDIR") {
            assert!(matches!(ex.check(&format!(r"{}\Temp", windir), "Temp"), ExclusionDecision::Never(_)));
        }
    }
}
