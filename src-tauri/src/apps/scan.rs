//! Builds the Unused apps list.
//!
//! Every input that touches the machine is passed in, so the logic can be tested with fakes.

use std::collections::HashMap;

use super::usage::{last_use_in, UsageStore};
use super::{AppInternal, AppKind, RemovalMethod, UnusedApp, UnusedAppsResult, UsageSource};
use crate::residual::evidence::{app_folder_for, is_ignored_location, is_installer_like, RunRecord};
use crate::residual::exclusions::{ExclusionDecision, ExclusionList};
use crate::residual::history::{app_key, InventoryHistory};
use crate::residual::inventory::{AppDetails, AppSource, InstalledApp};
use crate::residual::normalize::{exe_path_from_command, is_same_or_inside, parent_key, path_key};

/// Names of things that are not apps you "use": runtimes, drivers, SDKs, updates.
const EXCLUDED_NAME_WORDS: &[&str] = &[
    "runtime", "redistributable", "redist", "sdk", "driver", "framework", ".net", "visual c++",
    "webview2", "directx", "hotfix", "update for", "security update", "windows", "microsoft edge",
    "vcredist", "language pack", "bootstrapper", "prerequisite", "service pack", "java", "jdk",
    "jre", "corretto", "online services", "plugin", "add-in", "python launcher", "keyboard layout",
    "speech recognition", "device software", "product improvement", "gameinput", "service",
    "control panel", "controlpanel", "decoder",
];

/// Microsoft apps people actually open. Every other Microsoft entry is a component.
const MICROSOFT_APPS: &[&str] = &[
    "visual studio code", "visual studio community", "visual studio professional",
    "visual studio enterprise", "microsoft office", "microsoft 365", "onedrive", "microsoft teams",
];

/// Hardware vendors whose packages are drivers and control panels.
const EXCLUDED_PUBLISHER_WORDS: &[&str] = &[
    "nvidia", "intel", "advanced micro devices", "realtek", "qualcomm", "synaptics",
    "elan microelectronics", "dolby", "asao",
];

pub struct ScanInputs<'a> {
    pub installed: &'a [InstalledApp],
    pub details: &'a HashMap<String, AppDetails>,
    pub history: &'a InventoryHistory,
    pub usage: &'a UsageStore,
    pub runs: &'a [RunRecord],
    pub prefetch: Option<&'a HashMap<String, u64>>,
    /// Path keys of programs running now.
    pub running: &'a [String],
    /// Path keys of programs started by services, scheduled tasks or startup entries.
    pub referenced: &'a [String],
    /// Folders apps are installed under, for naming (Program Files, AppData, the profile, ...).
    pub roots: &'a [String],
    /// Program Files, Program Files (x86), ProgramData: removing from these needs admin.
    pub machine_roots: &'a [String],
    /// Folders that can never be an app's own folder (roots, the profile, Downloads, Windows).
    pub broad: &'a [String],
    /// Folders on PATH: command-line tools live here.
    pub path_dirs: &'a [String],
    pub profile: Option<&'a str>,
    pub windir: Option<&'a str>,
    pub exclusions: &'a ExclusionList,
    pub ignored_keys: &'a [String],
    pub exists: &'a dyn Fn(&str) -> bool,
    /// Program file names (lowercase) directly inside a folder and one level down.
    pub list_exes: &'a dyn Fn(&str) -> Vec<String>,
    /// Steam app id for a game folder under steamapps\common.
    pub steam_app_id: &'a dyn Fn(&str) -> Option<String>,
    pub scan_id: u64,
}

fn contains_word(haystack: &str, words: &[&str]) -> bool {
    let lower = haystack.to_lowercase();
    words.iter().any(|w| lower.contains(w))
}

fn is_guid(text: &str) -> bool {
    let t = text.trim();
    t.len() == 38
        && t.starts_with('{')
        && t.ends_with('}')
        && t[1..37].chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

struct Usage {
    best: Option<(u64, UsageSource)>,
}

impl Usage {
    fn new() -> Self {
        Self { best: None }
    }
    fn bump(&mut self, t: Option<u64>, src: UsageSource) {
        if let Some(t) = t {
            if t > 0 && self.best.map(|b| t > b.0).unwrap_or(true) {
                self.best = Some((t, src));
            }
        }
    }
}

impl<'a> ScanInputs<'a> {
    fn is_broad(&self, dir_key: &str) -> bool {
        dir_key.len() <= 3
            || self.broad.iter().any(|b| is_same_or_inside(b, dir_key))
            || self.windir.map(|w| is_same_or_inside(dir_key, w)).unwrap_or(false)
    }

    /// The folders an installed app owns, minus anything too broad to mean "this app".
    fn usable_dirs(&self, app: &InstalledApp) -> Vec<String> {
        app.owned_dirs()
            .into_iter()
            .filter(|d| !self.is_broad(d))
            .collect()
    }

    fn inside_any(&self, key: &str, dirs: &[String]) -> bool {
        dirs.iter().any(|d| is_same_or_inside(key, d))
    }

    fn needs_admin(&self, dir_key: &str) -> bool {
        self.machine_roots.iter().any(|r| is_same_or_inside(dir_key, r))
    }

    /// Last use of anything inside these folders, plus a Store app's launch id.
    fn last_use(&self, dirs: &[String], store_family: Option<&str>) -> Usage {
        Usage {
            best: last_use_in(self.runs, self.usage, self.prefetch, self.list_exes, dirs, store_family),
        }
    }

    fn excluded_registered(&self, app: &InstalledApp, details: Option<&AppDetails>) -> bool {
        let name = app.display_name.as_deref().unwrap_or("");
        let lower = name.to_lowercase();
        if app.hidden
            || name.trim().is_empty()
            || contains_word(name, EXCLUDED_NAME_WORDS)
            || lower.ends_with(" help")
        {
            return true;
        }
        if let Some(p) = app.publisher.as_deref() {
            if contains_word(p, EXCLUDED_PUBLISHER_WORDS) {
                return true;
            }
            if p.to_lowercase().contains("microsoft") && !contains_word(name, MICROSOFT_APPS) {
                return true;
            }
        }
        // Store publishers are often unreadable ids, so check vendor words in the name too.
        if app.source == AppSource::Store && contains_word(name, EXCLUDED_PUBLISHER_WORDS) {
            return true;
        }
        if app.source == AppSource::Store {
            let d = details.cloned().unwrap_or_default();
            if d.is_framework
                || d.non_removable
                || d.signature_kind.as_deref() != Some("Store")
                || app.publisher.as_deref().map(|p| p.to_lowercase().contains("microsoft")).unwrap_or(false)
            {
                return true;
            }
        }
        false
    }
}

/// "SpotifyAB.SpotifyMusic" reads better as "SpotifyMusic".
fn store_display_name(package_name: &str) -> String {
    match package_name.split_once('.') {
        Some((_, rest)) if !rest.is_empty() => rest.to_string(),
        _ => package_name.to_string(),
    }
}

/// Game folders managed by a launcher: `...\steamapps\common\<Game>` or `...\Epic Games\<Game>`.
fn launcher_game(full: &str) -> Option<(AppKind, String, String)> {
    let segments: Vec<&str> = full.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    for i in 0..segments.len().saturating_sub(2) {
        if segments[i].eq_ignore_ascii_case("steamapps") && segments[i + 1].eq_ignore_ascii_case("common") {
            let game = segments.get(i + 2)?;
            if i + 3 < segments.len() {
                return Some((AppKind::Steam, game.to_string(), segments[..i + 3].join("\\")));
            }
        }
    }
    for i in 0..segments.len().saturating_sub(1) {
        if segments[i].eq_ignore_ascii_case("epic games") {
            let game = segments.get(i + 1)?;
            if !game.eq_ignore_ascii_case("launcher") && i + 2 < segments.len() {
                return Some((AppKind::Epic, game.to_string(), segments[..i + 2].join("\\")));
            }
        }
    }
    None
}

pub fn build(inp: &ScanInputs) -> UnusedAppsResult {
    let mut apps: Vec<UnusedApp> = Vec::new();
    let mut considered = 0usize;
    let mut running_skipped = 0usize;
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Every folder any installed app owns, hidden ones included, so an unregistered program
    // inside one is never mistaken for a separate app.
    let all_owned: Vec<String> = inp.installed.iter().flat_map(|a| inp.usable_dirs(a)).collect();

    // ---- Registered and Store apps
    for app in inp.installed {
        let details = inp.details.get(&app.id);
        if inp.excluded_registered(app, details) {
            continue;
        }
        let history_key = app_key(app).unwrap_or_else(|| app.id.clone());
        if !seen_keys.insert(history_key.clone()) {
            continue; // the same app listed for both 64- and 32-bit
        }
        considered += 1;
        if inp.ignored_keys.iter().any(|k| k.eq_ignore_ascii_case(&history_key)) {
            continue;
        }
        let dirs = inp.usable_dirs(app);
        if inp.running.iter().any(|r| inp.inside_any(r, &dirs)) {
            running_skipped += 1;
            continue;
        }
        // A folder on PATH means a command-line tool, used from terminals and scripts
        // where no launch is ever recorded.
        if inp.path_dirs.iter().any(|p| inp.inside_any(p, &dirs)) {
            continue;
        }
        let usage = inp.last_use(&dirs, app.package_family_name.as_deref());
        let d = details.cloned().unwrap_or_default();
        let install_dir = app
            .install_location
            .as_deref()
            .map(|l| path_key(l.trim().trim_matches('"')))
            .filter(|l| !inp.is_broad(l) && (inp.exists)(l))
            .or_else(|| dirs.first().cloned().filter(|x| (inp.exists)(x)));

        let mut internal = AppInternal {
            registry_id: if app.source == AppSource::Store { None } else { Some(app.id.clone()) },
            history_key: Some(history_key.clone()),
            ..Default::default()
        };
        let (kind, method, fallback, note) = if app.source == AppSource::Store {
            internal.package_full_name = d.package_full_name.clone();
            if internal.package_full_name.is_none() {
                continue;
            }
            (AppKind::Store, RemovalMethod::StorePackage, None, None)
        } else {
            let product_code = d.registry_subkey.clone().filter(|s| is_guid(s));
            internal.msi_product_code = product_code.clone();
            let folder_fallback = install_dir.as_ref().map(|_| RemovalMethod::RemoveFolder);
            match app.uninstall_string.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(cmd) => {
                    internal.uninstall_string = Some(cmd.to_string());
                    let lower = cmd.to_lowercase();
                    let kind = if lower.contains("steam://uninstall") { AppKind::Steam } else { AppKind::Registered };
                    if lower.contains("msiexec") {
                        match product_code {
                            Some(_) => (kind, RemovalMethod::MsiProduct, None, None),
                            None => (kind, RemovalMethod::Uninstaller, None, None),
                        }
                    } else if lower.contains("://") {
                        (kind, RemovalMethod::Uninstaller, None, None)
                    } else {
                        match exe_path_from_command(cmd) {
                            Some(exe) if (inp.exists)(&exe) => (kind, RemovalMethod::Uninstaller, None, None),
                            _ => match product_code {
                                Some(_) => (
                                    kind,
                                    RemovalMethod::MsiProduct,
                                    None,
                                    Some("The app's uninstaller is missing. Windows Installer can remove it instead.".to_string()),
                                ),
                                None => (
                                    kind,
                                    RemovalMethod::WindowsSettings,
                                    folder_fallback,
                                    Some("The app's uninstaller is missing. Try Windows Settings, or remove its program folder.".to_string()),
                                ),
                            },
                        }
                    }
                }
                None => match product_code {
                    Some(_) => (AppKind::Registered, RemovalMethod::MsiProduct, None, None),
                    None => (
                        AppKind::Registered,
                        RemovalMethod::WindowsSettings,
                        folder_fallback,
                        Some("This app has no uninstaller. Try Windows Settings, or remove its program folder.".to_string()),
                    ),
                },
            }
        };

        let name = if app.source == AppSource::Store {
            store_display_name(app.display_name.as_deref().unwrap_or(""))
        } else {
            app.display_name.clone().unwrap_or_default()
        };
        // Store publishers are often a bare id ("CN=ED346674-0FA1-..."): not worth showing.
        let publisher = app.publisher.clone().filter(|p| {
            let hexish = p.chars().filter(|c| c.is_ascii_hexdigit() || *c == '-').count();
            !(hexish == p.len() && p.contains('-'))
        });
        apps.push(UnusedApp {
            id: app.id.clone(),
            key: history_key,
            name,
            publisher,
            kind,
            method,
            fallback,
            note,
            last_used_ms: usage.best.map(|b| b.0),
            last_used_source: usage.best.map(|b| b.1),
            // Only the app's own install date: ASAO's first sighting is not when it was installed.
            installed_ms: d.install_date_ms,
            size_bytes: d.estimated_size_bytes,
            needs_admin: app.source != AppSource::Store
                && (matches!(app.source, AppSource::RegistryMachine | AppSource::RegistryMachineWow64)
                    || install_dir.as_deref().map(|x| inp.needs_admin(x)).unwrap_or(false)),
            install_dir,
            internal,
        });
    }

    // ---- Apps that were never registered: programs seen running or launched, grouped by folder
    struct Group {
        kind: AppKind,
        name: String,
        dir: String,
        usage: Usage,
    }
    let mut groups: HashMap<String, Group> = HashMap::new();
    let mut candidates: Vec<(String, Option<u64>, UsageSource)> = Vec::new();
    for r in inp.runs.iter().filter(|r| r.is_program) {
        candidates.push((r.target.clone(), r.last_run_ms, UsageSource::LaunchRecord));
    }
    for (key, t) in &inp.usage.last_seen {
        let full = inp.usage.paths.get(key).cloned().unwrap_or_else(|| key.clone());
        candidates.push((full, Some(*t), UsageSource::AsaoMonitor));
    }
    for (full, t, src) in candidates {
        let key = path_key(&full);
        if inp.windir.map(|w| is_same_or_inside(&key, w)).unwrap_or(false) || is_ignored_location(&key) {
            continue;
        }
        if let Some(profile) = inp.profile {
            if is_same_or_inside(&key, profile) && !key.contains("\\appdata\\") {
                continue;
            }
        }
        if let Some(dir) = parent_key(&key) {
            if inp.path_dirs.iter().any(|p| p == &dir) {
                continue; // a command-line tool on PATH
            }
        }
        if inp.inside_any(&key, &all_owned) || !(inp.exists)(&full) {
            continue;
        }
        let (kind, name, dir) = match launcher_game(&full) {
            Some(g) => g,
            None => {
                let Some(af) = app_folder_for(&full, inp.roots) else { continue };
                if is_installer_like(&af.exe_stem) || is_installer_like(&af.name) {
                    continue;
                }
                (AppKind::Unregistered, af.name, af.dir)
            }
        };
        let dir_key = path_key(&dir);
        if inp.is_broad(&dir_key) || name.starts_with('.') {
            continue;
        }
        if let ExclusionDecision::Never(_) = inp.exclusions.check(&dir, &name) {
            continue;
        }
        let g = groups.entry(dir_key.clone()).or_insert_with(|| Group {
            kind,
            name: name.clone(),
            dir: dir.clone(),
            usage: Usage::new(),
        });
        g.usage.bump(t, src);
    }

    for (dir_key, g) in groups {
        considered += 1;
        let key = format!("folder|{}", dir_key);
        if inp.ignored_keys.iter().any(|k| k.eq_ignore_ascii_case(&key)) {
            continue;
        }
        let dirs = vec![dir_key.clone()];
        if inp.running.iter().any(|r| inp.inside_any(r, &dirs)) {
            running_skipped += 1;
            continue;
        }
        if inp.referenced.iter().any(|r| inp.inside_any(r, &dirs)) {
            continue; // started by a service, task or startup entry: still in use
        }
        let mut usage = g.usage;
        if let Some(prefetch) = inp.prefetch {
            for name in (inp.list_exes)(&dir_key).into_iter().take(60) {
                usage.bump(prefetch.get(&name).copied(), UsageSource::Prefetch);
            }
        }
        let mut internal = AppInternal::default();
        let (method, note) = match g.kind {
            AppKind::Steam => match (inp.steam_app_id)(&g.dir) {
                Some(id) => {
                    internal.steam_app_id = Some(id);
                    (RemovalMethod::SteamUninstall, None)
                }
                None => continue, // without its id, Steam would be left confused
            },
            AppKind::Epic => (
                RemovalMethod::OpenEpic,
                Some("Epic Games Launcher manages this game. Uninstall it from your library.".to_string()),
            ),
            _ => (
                RemovalMethod::RemoveFolder,
                Some("Not registered with Windows, so there is no uninstaller. ASAO can move its program folder to the Recycle Bin.".to_string()),
            ),
        };
        apps.push(UnusedApp {
            id: key.clone(),
            key,
            name: g.name,
            publisher: None,
            kind: g.kind,
            method,
            fallback: None,
            note,
            last_used_ms: usage.best.map(|b| b.0),
            last_used_source: usage.best.map(|b| b.1),
            installed_ms: None,
            size_bytes: None,
            needs_admin: inp.needs_admin(&dir_key),
            install_dir: Some(g.dir),
            internal,
        });
    }

    // Longest unused first; apps with no record of use after them, by name.
    apps.sort_by(|a, b| match (a.last_used_ms, b.last_used_ms) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    UnusedAppsResult {
        scan_id: inp.scan_id,
        apps,
        prefetch_available: inp.prefetch.is_some(),
        tracking_since_ms: if inp.usage.since_ms > 0 { Some(inp.usage.since_ms) } else { None },
        considered,
        running_skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(id: &str, name: &str, publisher: &str, loc: Option<&str>, uninstall: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: id.into(),
            display_name: Some(name.into()),
            publisher: Some(publisher.into()),
            install_location: loc.map(|s| s.to_string()),
            uninstall_string: uninstall.map(|s| s.to_string()),
            display_icon: None,
            source: AppSource::RegistryUser,
            hidden: false,
            package_family_name: None,
            package_name: None,
        }
    }

    struct World {
        installed: Vec<InstalledApp>,
        details: HashMap<String, AppDetails>,
        history: InventoryHistory,
        usage: UsageStore,
        runs: Vec<RunRecord>,
        running: Vec<String>,
        exclusions: ExclusionList,
    }

    fn world() -> World {
        let mut details = HashMap::new();
        details.insert(
            "hkcu:{11111111-2222-3333-4444-555555555555}".to_string(),
            AppDetails { registry_subkey: Some("{11111111-2222-3333-4444-555555555555}".into()), ..Default::default() },
        );
        World {
            installed: vec![
                reg("hkcu:old", "Old Editor", "Contoso", Some(r"C:\Program Files\Old Editor"), Some(r#""C:\Program Files\Old Editor\unins000.exe""#)),
                reg("hkcu:recent", "Recent Player", "Fabrikam", Some(r"C:\Program Files\Recent Player"), Some(r#""C:\Program Files\Recent Player\uninstall.exe""#)),
                reg("hkcu:broken", "Broken Tool", "Northwind", Some(r"C:\Program Files\Broken Tool"), Some(r#""C:\Program Files\Broken Tool\gone.exe""#)),
                reg("hkcu:{11111111-2222-3333-4444-555555555555}", "Msi Thing", "Litware", None, Some("MsiExec.exe /I{11111111-2222-3333-4444-555555555555}")),
                reg("hkcu:running", "Busy App", "Tailspin", Some(r"C:\Program Files\Busy App"), Some(r#""C:\Program Files\Busy App\u.exe""#)),
                reg("hkcu:rt", "Microsoft Visual C++ 2022 Redistributable", "Microsoft Corporation", None, Some("x")),
                reg("hkcu:drv", "Graphics Control Panel", "NVIDIA Corporation", None, Some("x")),
                reg("hkcu:mscomp", "Microsoft Update Health Tools", "Microsoft Corporation", None, Some("x")),
                reg("hkcu:vscode", "Microsoft Visual Studio Code (User)", "Microsoft Corporation", Some(r"C:\Users\x\AppData\Local\Programs\VS Code"), Some(r#""C:\Users\x\AppData\Local\Programs\VS Code\unins000.exe""#)),
                reg("hkcu:git", "Git", "The Git Development Community", Some(r"C:\Program Files\Git"), Some(r#""C:\Program Files\Git\unins000.exe""#)),
            ],
            details,
            history: InventoryHistory::default(),
            usage: UsageStore::default(),
            runs: vec![
                RunRecord { target: r"C:\Program Files\Old Editor\editor.exe".into(), is_program: true, last_run_ms: Some(1_000) },
                RunRecord { target: r"C:\Program Files\Recent Player\player.exe".into(), is_program: true, last_run_ms: Some(9_000) },
                // A portable app nobody registered
                RunRecord { target: r"C:\Tools\PortaPaint\portapaint.exe".into(), is_program: true, last_run_ms: Some(2_000) },
                // An installer: never an app
                RunRecord { target: r"C:\Tools\Setups\thing_setup.exe".into(), is_program: true, last_run_ms: Some(3_000) },
                // A Steam game
                RunRecord { target: r"D:\SteamLibrary\steamapps\common\Space Game\bin\space.exe".into(), is_program: true, last_run_ms: Some(4_000) },
            ],
            running: vec![r"c:\program files\busy app\busy.exe".into()],
            exclusions: ExclusionList::new(vec![]),
        }
    }

    fn run(w: &World) -> UnusedAppsResult {
        let exists = |p: &str| !p.to_lowercase().contains("gone.exe");
        let list_exes = |_: &str| Vec::<String>::new();
        let steam = |dir: &str| if dir.ends_with("Space Game") { Some("440".to_string()) } else { None };
        let roots = vec![r"c:\program files".to_string()];
        let broad = vec![r"c:\program files".to_string(), r"c:\users\x".to_string()];
        build(&ScanInputs {
            installed: &w.installed,
            details: &w.details,
            history: &w.history,
            usage: &w.usage,
            runs: &w.runs,
            prefetch: None,
            running: &w.running,
            referenced: &[],
            roots: &roots,
            machine_roots: &[r"c:\program files".to_string()],
            broad: &broad,
            path_dirs: &[r"c:\program files\git\cmd".to_string()],
            profile: Some(r"c:\users\x"),
            windir: Some(r"c:\windows"),
            exclusions: &w.exclusions,
            ignored_keys: &[],
            exists: &exists,
            list_exes: &list_exes,
            steam_app_id: &steam,
            scan_id: 1,
        })
    }

    fn find<'a>(r: &'a UnusedAppsResult, name: &str) -> Option<&'a UnusedApp> {
        r.apps.iter().find(|a| a.name == name)
    }

    #[test]
    fn lists_apps_with_last_use_and_the_right_method() {
        let r = run(&world());
        let old = find(&r, "Old Editor").unwrap();
        assert_eq!(old.last_used_ms, Some(1_000));
        assert_eq!(old.last_used_source, Some(UsageSource::LaunchRecord));
        assert_eq!(old.method, RemovalMethod::Uninstaller);

        let broken = find(&r, "Broken Tool").unwrap();
        assert_eq!(broken.method, RemovalMethod::WindowsSettings);
        assert_eq!(broken.fallback, Some(RemovalMethod::RemoveFolder));
        assert!(broken.note.is_some());

        let msi = find(&r, "Msi Thing").unwrap();
        assert_eq!(msi.method, RemovalMethod::MsiProduct);
        assert_eq!(msi.internal.msi_product_code.as_deref(), Some("{11111111-2222-3333-4444-555555555555}"));

        // Oldest use first
        assert_eq!(r.apps[0].name, "Old Editor");
    }

    #[test]
    fn skips_running_runtimes_drivers_and_installers() {
        let r = run(&world());
        assert!(find(&r, "Busy App").is_none());
        assert_eq!(r.running_skipped, 1);
        assert!(find(&r, "Microsoft Visual C++ 2022 Redistributable").is_none());
        assert!(find(&r, "Graphics Control Panel").is_none());
        assert!(r.apps.iter().all(|a| !a.name.to_lowercase().contains("setup")));
        // Microsoft components are left out, Microsoft apps people open are kept
        assert!(find(&r, "Microsoft Update Health Tools").is_none());
        assert!(find(&r, "Microsoft Visual Studio Code (User)").is_some());
        // Command-line tools (a folder on PATH) are left out
        assert!(find(&r, "Git").is_none());
    }

    #[test]
    fn unregistered_apps_and_launcher_games() {
        let r = run(&world());
        let porta = find(&r, "PortaPaint").unwrap();
        assert_eq!(porta.kind, AppKind::Unregistered);
        assert_eq!(porta.method, RemovalMethod::RemoveFolder);
        assert_eq!(porta.last_used_ms, Some(2_000));

        let game = find(&r, "Space Game").unwrap();
        assert_eq!(game.kind, AppKind::Steam);
        assert_eq!(game.method, RemovalMethod::SteamUninstall);
        assert_eq!(game.internal.steam_app_id.as_deref(), Some("440"));
        assert_eq!(game.install_dir.as_deref(), Some(r"D:\SteamLibrary\steamapps\common\Space Game"));
    }

    #[test]
    fn ignored_apps_are_left_out() {
        let w = world();
        let exists = |_: &str| true;
        let list_exes = |_: &str| Vec::<String>::new();
        let steam = |_: &str| None;
        let ignored = vec!["old editor|contoso".to_string()];
        let r = build(&ScanInputs {
            installed: &w.installed,
            details: &w.details,
            history: &w.history,
            usage: &w.usage,
            runs: &w.runs,
            prefetch: None,
            running: &[],
            referenced: &[],
            roots: &[],
            machine_roots: &[],
            broad: &[],
            path_dirs: &[],
            profile: None,
            windir: None,
            exclusions: &w.exclusions,
            ignored_keys: &ignored,
            exists: &exists,
            list_exes: &list_exes,
            steam_app_id: &steam,
            scan_id: 2,
        });
        assert!(find(&r, "Old Editor").is_none());
    }

    #[test]
    fn store_names_and_guids() {
        assert_eq!(store_display_name("SpotifyAB.SpotifyMusic"), "SpotifyMusic");
        assert_eq!(store_display_name("Plain"), "Plain");
        assert!(is_guid("{11111111-2222-3333-4444-555555555555}"));
        assert!(!is_guid("Notepad++"));
    }
}

#[cfg(test)]
mod live {
    use super::*;
    use crate::residual::normalize::path_key;

    /// Read-only: prints what the Unused apps tab would list on this machine.
    /// Run with: cargo test --lib live_unused_apps_smoke -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_unused_apps_smoke() {
        let inv = crate::residual::inventory::collect_installed_apps();
        let runs = crate::residual::evidence::userassist_records();
        let prefetch = crate::apps::usage::prefetch_last_runs();
        let referenced = crate::residual::references::collect_referenced_executables().unwrap_or_default();
        let known = crate::residual::roots::known_roots();
        let mut roots: Vec<String> = known.iter().map(|r| path_key(&r.path.to_string_lossy())).collect();
        let profile = std::env::var("USERPROFILE").ok().map(|p| path_key(&p));
        let mut broad = roots.clone();
        if let Some(p) = &profile {
            roots.push(p.clone());
            broad.push(p.clone());
            for sub in ["appdata", "downloads", "desktop", "documents", r"appdata\local\programs"] {
                roots.push(format!("{}\\{}", p, sub));
                broad.push(format!("{}\\{}", p, sub));
            }
        }
        let windir = std::env::var("WINDIR").ok().map(|w| path_key(&w));
        let path_dirs: Vec<String> = std::env::var("PATH").map(|p| p.split(';').map(path_key).collect()).unwrap_or_default();
        let exclusions = ExclusionList::new(vec![]);
        let exists = |x: &str| std::path::Path::new(x).exists();
        let list_exes = |_: &str| Vec::<String>::new();
        let steam = |d: &str| crate::apps::actions::steam_app_id_for(d);
        let r = build(&ScanInputs {
            installed: &inv.apps,
            details: &inv.details,
            history: &InventoryHistory::default(),
            usage: &UsageStore::default(),
            runs: &runs,
            prefetch: prefetch.as_ref(),
            running: &[],
            referenced: &referenced,
            roots: &roots,
            machine_roots: &[],
            broad: &broad,
            path_dirs: &path_dirs,
            profile: profile.as_deref(),
            windir: windir.as_deref(),
            exclusions: &exclusions,
            ignored_keys: &[],
            exists: &exists,
            list_exes: &list_exes,
            steam_app_id: &steam,
            scan_id: 0,
        });
        println!("considered {} listed {} prefetch {}", r.considered, r.apps.len(), r.prefetch_available);
        let now = crate::residual::now_ms();
        for a in &r.apps {
            let days = a.last_used_ms.map(|t| (now.saturating_sub(t)) / 86_400_000);
            println!("{:?}\t{:?}\tdays={:?}\t{:?}\t{}\t{:?}", a.kind, a.method, days, a.last_used_source, a.name, a.install_dir);
        }
    }
}
