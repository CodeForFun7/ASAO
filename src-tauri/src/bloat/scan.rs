//! Builds the reviewed list: one row per app with its services, startup entries and
//! scheduled tasks underneath, plus rows for services and startup entries no app owns.
//! Pure: every input is passed in, so tests can run it on fixed data.

use std::collections::{HashMap, HashSet};

use super::database::{BloatwareDatabase, ConditionFacts, DbEntry, ServiceAction};
use super::impact::{score, ImpactInput, Samples};
use super::inventory::exe_of_command;
use super::safety::{always_kept, apply_runtime_checks, base_verdict, preselected, RuntimeFacts};
use super::{
    BloatItem, Category, DeviceContext, Impact, ImpactBand, ImpactPart, ItemKind, Safety, ServiceInfo,
    StartupEntryInfo, TaskInfo,
};
use crate::residual::inventory::{AppDetails, AppSource, InstalledApp};
use crate::residual::normalize::{is_same_or_inside, path_key};

pub struct ScanInputs<'a> {
    pub db: &'a BloatwareDatabase,
    pub apps: &'a [InstalledApp],
    pub details: &'a HashMap<String, AppDetails>,
    pub services: &'a [ServiceInfo],
    pub startup: &'a [StartupEntryInfo],
    pub tasks: &'a [TaskInfo],
    pub device: &'a DeviceContext,
    pub samples: &'a Samples,
    /// Windows' startup measurements per program path key (CPU ms, disk bytes).
    pub startup_info: Option<&'a HashMap<String, (u64, u64)>>,
    /// Folders too broad to mean one app (Program Files itself, the profile, Windows).
    pub broad: &'a [String],
    pub windir: Option<&'a str>,
    /// Last use of anything in these folders, or of this Store family.
    pub last_use: &'a dyn Fn(&[String], Option<&str>) -> Option<u64>,
    /// Size of a folder, measured on demand.
    pub folder_size: &'a dyn Fn(&str) -> Option<u64>,
    pub now_ms: u64,
}

struct Draft {
    item: BloatItem,
    dirs: Vec<String>,
    entry_id: Option<String>,
    is_framework: bool,
    package_name: Option<String>,
    protected: Option<String>,
    estimated_size: Option<u64>,
}

fn empty_impact() -> Impact {
    Impact {
        total: 0,
        band: ImpactBand::Low,
        startup: ImpactPart::default(),
        cpu: ImpactPart::default(),
        memory: ImpactPart::default(),
        autostart: ImpactPart::default(),
        disk: ImpactPart::default(),
    }
}

fn blank(id: String, kind: ItemKind, name: String) -> BloatItem {
    BloatItem {
        id,
        kind,
        name,
        publisher: None,
        category: None,
        db_id: None,
        what_it_does: None,
        what_breaks: None,
        how_to_restore: None,
        source: None,
        recommendation: None,
        safety: Safety::NotRated,
        safety_reason: String::new(),
        safety_checks: Vec::new(),
        selectable: false,
        preselected: false,
        package_family_name: None,
        package_full_name: None,
        package_name: None,
        registry_subkey: None,
        install_location: None,
        size_bytes: None,
        services: Vec::new(),
        startup: Vec::new(),
        tasks: Vec::new(),
        dependents: Vec::new(),
        running: false,
        last_used_ms: None,
        impact: empty_impact(),
        planned_changes: Vec::new(),
        undo: String::new(),
        store_url: None,
    }
}

fn apply_entry(item: &mut BloatItem, e: &DbEntry) {
    item.db_id = Some(e.id.clone());
    item.category = Some(e.category);
    item.what_it_does = Some(e.what_it_does.clone());
    item.what_breaks = Some(e.what_breaks.clone());
    item.how_to_restore = Some(e.how_to_restore.clone());
    item.source = Some(e.source.clone());
    if item.kind != ItemKind::Win32App {
        item.name = e.display_name.clone();
    }
}

/// "E2B691C9.C-Media6549AudioControlPanel" → "C-Media6549AudioControlPanel".
fn readable_package_name(name: &str) -> String {
    match name.split_once('.') {
        Some((prefix, rest))
            if !rest.is_empty() && prefix.len() >= 6 && prefix.chars().all(|c| c.is_ascii_hexdigit()) =>
        {
            rest.to_string()
        }
        _ => name.to_string(),
    }
}

impl<'a> ScanInputs<'a> {
    fn is_broad(&self, dir: &str) -> bool {
        dir.len() <= 3
            || self.broad.iter().any(|b| is_same_or_inside(b, dir))
            || self.windir.map(|w| is_same_or_inside(dir, w)).unwrap_or(false)
    }

    fn facts(&self) -> ConditionFacts {
        ConditionFacts {
            games_installed: self.device.games_installed,
            org_managed: self.device.org_managed,
        }
    }

    fn app_drafts(&self) -> Vec<Draft> {
        let mut out = Vec::new();
        // The same Win32 app can be registered in both the 64-bit and 32-bit views.
        let mut seen_win32: HashSet<(String, String)> = HashSet::new();
        for app in self.apps {
            if app.source != AppSource::Store {
                let key = (
                    app.display_name.clone().unwrap_or_default().to_lowercase(),
                    app.publisher.clone().unwrap_or_default().to_lowercase(),
                );
                if !key.0.is_empty() && !seen_win32.insert(key) {
                    continue;
                }
            }
            let details = self.details.get(&app.id).cloned().unwrap_or_default();
            let is_store = app.source == AppSource::Store;
            let raw_name = app.display_name.clone().unwrap_or_default();
            let package_name = app.package_name.clone();
            let entry = if is_store {
                package_name.as_deref().and_then(|p| self.db.match_package(p))
            } else if !raw_name.trim().is_empty() {
                self.db.match_win32(&raw_name, app.publisher.as_deref())
            } else {
                None
            };
            // Hidden pieces (frameworks, system packages, SystemComponent entries) are only
            // listed when the database knows them.
            let system_package = is_store
                && (details.non_removable || details.signature_kind.as_deref() == Some("System"));
            let nameless = !is_store && raw_name.trim().is_empty();
            if entry.is_none() && (app.hidden || details.is_framework || system_package || nameless) {
                continue;
            }
            let name = if is_store {
                package_name.as_deref().map(readable_package_name).unwrap_or(raw_name.clone())
            } else {
                raw_name.clone()
            };
            let kind = if is_store { ItemKind::StoreApp } else { ItemKind::Win32App };
            let id = if is_store {
                format!("store:{}", app.package_family_name.clone().unwrap_or_else(|| app.id.clone()))
            } else {
                format!("app:{}", app.id)
            };
            let mut item = blank(id, kind, name);
            // Store packages sometimes report a certificate id instead of a name.
            item.publisher = app.publisher.clone().filter(|p| {
                let core: String = p.chars().filter(|c| *c != '-' && *c != '{' && *c != '}').collect();
                !(core.len() >= 16 && core.chars().all(|c| c.is_ascii_hexdigit()))
            });
            item.package_family_name = app.package_family_name.clone();
            item.package_full_name = details.package_full_name.clone();
            item.package_name = package_name.clone();
            item.registry_subkey = details.registry_subkey.clone();
            item.install_location = app.install_location.clone();
            if let Some(e) = entry {
                apply_entry(&mut item, e);
            }
            let dirs: Vec<String> = app.owned_dirs().into_iter().filter(|d| !self.is_broad(d)).collect();
            let protected = if system_package {
                Some("Windows does not allow removing this package.".to_string())
            } else {
                None
            };
            out.push(Draft {
                item,
                dirs,
                entry_id: entry.map(|e| e.id.clone()),
                is_framework: details.is_framework,
                package_name,
                protected,
                estimated_size: details.estimated_size_bytes,
            });
        }
        out
    }

    /// The draft that owns a program path: the deepest app folder containing it.
    fn owner_of(&self, drafts: &[Draft], exe: Option<&str>) -> Option<usize> {
        let key = path_key(exe?);
        drafts
            .iter()
            .enumerate()
            .filter_map(|(i, d)| {
                d.dirs
                    .iter()
                    .filter(|dir| is_same_or_inside(&key, dir))
                    .map(|dir| dir.len())
                    .max()
                    .map(|len| (i, len))
            })
            .max_by_key(|(_, len)| *len)
            .map(|(i, _)| i)
    }

    fn by_entry(drafts: &[Draft], entry: Option<&DbEntry>) -> Option<usize> {
        let id = &entry?.id;
        drafts.iter().position(|d| d.entry_id.as_deref() == Some(id.as_str()))
    }

    pub fn build(&self) -> Vec<BloatItem> {
        let facts = self.facts();
        let mut drafts = self.app_drafts();
        let app_count = drafts.len();

        // A folder several apps point at (Chrome web apps all name Chrome's folder) only
        // belongs to the app installed there, so its processes are not counted for every one.
        let mut dir_count: HashMap<String, usize> = HashMap::new();
        for d in &drafts {
            for dir in &d.dirs {
                *dir_count.entry(dir.clone()).or_default() += 1;
            }
        }
        for d in &mut drafts {
            let own = d.item.install_location.as_deref().map(|l| path_key(l.trim().trim_matches('"')));
            d.dirs
                .retain(|dir| dir_count.get(dir).copied().unwrap_or(0) <= 1 || own.as_deref() == Some(dir.as_str()));
        }

        // Services: to their app (by folder or by database entry), else standalone when
        // the database knows them or they are not part of Windows.
        let names: HashSet<String> = self.services.iter().map(|s| s.name.to_lowercase()).collect();
        for s in self.services {
            // Per-user service templates ("CDPUserSvc" next to "CDPUserSvc_1a2b3") are skipped.
            let lower = s.name.to_lowercase();
            if names.iter().any(|n| n.len() > lower.len() + 1 && n.starts_with(&format!("{}_", lower))) {
                continue;
            }
            let entry = self.db.match_service(&s.name);
            let mut svc = s.clone();
            if let Some(e) = entry {
                svc.planned_start_type = match e.service_action {
                    ServiceAction::Disabled => "Disabled".into(),
                    ServiceAction::Manual => "Manual".into(),
                };
            }
            let exe = s.binary_path.as_deref().and_then(exe_of_command);
            let owner = self
                .owner_of(&drafts[..app_count], exe.as_deref())
                .or_else(|| Self::by_entry(&drafts[..app_count], entry));
            match owner {
                Some(i) => drafts[i].item.services.push(svc),
                None if entry.is_some() || !s.is_windows => {
                    let mut item = blank(format!("svc:{}", s.name), ItemKind::Service, s.display_name.clone());
                    item.what_it_does = s.description.clone();
                    if let Some(e) = entry {
                        apply_entry(&mut item, e);
                    }
                    item.services.push(svc);
                    drafts.push(Draft {
                        item,
                        dirs: Vec::new(),
                        entry_id: entry.map(|e| e.id.clone()),
                        is_framework: false,
                        package_name: None,
                        protected: None,
                        estimated_size: None,
                    });
                }
                None => {}
            }
        }

        // Startup entries: to their app, else standalone unless they run something in Windows.
        for st in self.startup {
            let entry = self.db.match_startup(&st.name);
            let mut info = st.clone();
            if let (Some(map), Some(exe)) = (self.startup_info, st.executable.as_deref()) {
                if let Some((cpu, disk)) = map.get(&path_key(exe)) {
                    info.boot_cpu_ms = Some(*cpu);
                    info.boot_disk_bytes = Some(*disk);
                }
            }
            let owner = self
                .owner_of(&drafts[..app_count], st.executable.as_deref())
                .or_else(|| Self::by_entry(&drafts[..app_count], entry));
            match owner {
                Some(i) => drafts[i].item.startup.push(info),
                None => {
                    let in_windows = st
                        .executable
                        .as_deref()
                        .map(|e| self.windir.map(|w| is_same_or_inside(&path_key(e), w)).unwrap_or(false))
                        .unwrap_or(false);
                    if in_windows && entry.is_none() {
                        continue;
                    }
                    let mut item = blank(format!("run:{}", st.id), ItemKind::StartupEntry, st.name.clone());
                    if let Some(e) = entry {
                        apply_entry(&mut item, e);
                    }
                    item.startup.push(info);
                    drafts.push(Draft {
                        item,
                        dirs: Vec::new(),
                        entry_id: entry.map(|e| e.id.clone()),
                        is_framework: false,
                        package_name: None,
                        protected: None,
                        estimated_size: None,
                    });
                }
            }
        }

        // Scheduled tasks: only those that belong to an app or a database entry.
        for t in self.tasks {
            let entry = self.db.match_task(&t.path);
            let exe = t.action.as_deref().and_then(exe_of_command);
            let owner = self
                .owner_of(&drafts[..app_count], exe.as_deref())
                .or_else(|| Self::by_entry(&drafts, entry));
            if let Some(i) = owner {
                drafts[i].item.tasks.push(t.clone());
            }
        }

        let services_by_name: HashMap<String, &ServiceInfo> =
            self.services.iter().map(|s| (s.name.to_lowercase(), s)).collect();
        let active_av: Vec<String> = self
            .device
            .antivirus
            .iter()
            .filter(|a| a.enabled)
            .map(|a| a.name.clone())
            .collect();
        let runtime = RuntimeFacts {
            conditions: facts,
            active_antivirus: &active_av,
            defender_present: self.device.defender_present,
            managed_reason: self.device.managed_reason.as_deref(),
            now_ms: self.now_ms,
        };

        let mut out = Vec::new();
        for d in drafts {
            let Draft { mut item, dirs, entry_id, is_framework, package_name, protected, estimated_size } = d;
            let entry = entry_id.as_deref().and_then(|id| self.db.find(id));

            // Dependents outside this item.
            let own: HashSet<String> = item.services.iter().map(|s| s.name.to_lowercase()).collect();
            let mut dependents = Vec::new();
            for s in &item.services {
                for dep in &s.dependents {
                    if own.contains(&dep.to_lowercase()) {
                        continue;
                    }
                    let active = services_by_name
                        .get(&dep.to_lowercase())
                        .map(|x| !x.start_type.eq_ignore_ascii_case("Disabled"))
                        .unwrap_or(true);
                    if active {
                        let label = services_by_name
                            .get(&dep.to_lowercase())
                            .map(|x| format!("{} ({})", x.display_name, x.name))
                            .unwrap_or_else(|| dep.clone());
                        if !dependents.contains(&label) {
                            dependents.push(label);
                        }
                    }
                }
            }
            item.dependents = dependents;

            // Processes: its services' own processes, and programs inside its folders.
            let service_pids: HashSet<u32> = item
                .services
                .iter()
                .filter(|s| s.process_id != 0 && !is_shared_host(s))
                .map(|s| s.process_id)
                .collect();
            let service_exes: HashSet<String> = item
                .services
                .iter()
                .filter(|s| !is_shared_host(s))
                .filter_map(|s| s.binary_path.as_deref().and_then(exe_of_command).map(|e| path_key(&e)))
                .collect();
            let startup_exes: HashSet<String> = item
                .startup
                .iter()
                .filter_map(|s| s.executable.as_deref().map(path_key))
                .collect();
            let belongs = |pid: u32, exe: Option<&str>| {
                service_pids.contains(&pid)
                    || exe
                        .map(|e| {
                            service_exes.contains(e)
                                || startup_exes.contains(e)
                                || dirs.iter().any(|dir| is_same_or_inside(e, dir))
                        })
                        .unwrap_or(false)
            };
            let (cpu, memory) = self.samples.totals(&belongs);
            // "Running" means a program the user sees, not one of its background services.
            item.running = self.samples.by_pid.iter().any(|(pid, s)| {
                !service_pids.contains(pid)
                    && s.exe_key
                        .as_deref()
                        .map(|e| !service_exes.contains(e) && dirs.iter().any(|dir| is_same_or_inside(e, dir)))
                        .unwrap_or(false)
            });
            if !dirs.is_empty() || item.package_family_name.is_some() {
                item.last_used_ms = (self.last_use)(&dirs, item.package_family_name.as_deref());
            }

            // Size: the registry estimate, or the folder measured (only for items the database
            // knows and Store apps, to keep the scan quick).
            item.size_bytes = estimated_size.filter(|b| *b > 0).or_else(|| {
                let worth = entry.is_some() || item.kind == ItemKind::StoreApp;
                if worth {
                    dirs.first().and_then(|d| (self.folder_size)(d)).filter(|b| *b > 0)
                } else {
                    None
                }
            });

            let enabled_startup: Vec<Option<(u64, u64)>> = item
                .startup
                .iter()
                .filter(|s| s.enabled)
                .map(|s| s.boot_cpu_ms.zip(s.boot_disk_bytes))
                .collect();
            let enabled_tasks: Vec<&TaskInfo> = item.tasks.iter().filter(|t| t.enabled).collect();
            item.impact = score(&ImpactInput {
                startup: enabled_startup,
                logon_tasks: enabled_tasks
                    .iter()
                    .filter(|t| t.triggers.iter().any(|x| x.starts_with("At sign-in") || x.starts_with("At startup")))
                    .count() as u32,
                cpu_percent: cpu,
                sample_seconds: self.samples.seconds,
                memory_bytes: memory,
                auto_services: item
                    .services
                    .iter()
                    .filter(|s| s.start_type.eq_ignore_ascii_case("Auto"))
                    .count() as u32,
                frequent_tasks: enabled_tasks.iter().filter(|t| t.frequent).count() as u32,
                disk_bytes: item.size_bytes,
            });
            if item.services.iter().any(is_shared_host) && cpu.is_none() {
                item.impact.cpu.value = "Runs inside a shared Windows service host; not measured on its own".into();
            }

            // Safety.
            let kept = always_kept(item.kind, package_name.as_deref(), &item.name, is_framework);
            let mut verdict = base_verdict(entry, &runtime);
            apply_runtime_checks(&mut verdict, &item, &runtime, kept, protected);
            item.safety = verdict.safety;
            item.safety_reason = verdict.reason;
            item.safety_checks = verdict.checks;
            item.recommendation = verdict.recommendation;
            item.selectable = item.safety.selectable();
            item.preselected = preselected(&item, entry.map(|e| e.selected_by_default).unwrap_or(false));

            describe_changes(&mut item);
            out.push(item);
        }

        out.sort_by(|a, b| b.impact.total.cmp(&a.impact.total).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        out
    }
}

/// A Windows service running inside a shared `svchost.exe`.
fn is_shared_host(s: &ServiceInfo) -> bool {
    s.binary_path
        .as_deref()
        .map(|p| p.to_lowercase().contains("svchost.exe"))
        .unwrap_or(false)
}

/// What ASAO would change, in the order it would do it, and how it can be undone.
pub fn describe_changes(item: &mut BloatItem) {
    let mut changes = Vec::new();
    for s in item.startup.iter().filter(|s| s.enabled) {
        changes.push(format!("Disable the startup entry \"{}\" (the same switch as Task Manager)", s.name));
    }
    for t in item.tasks.iter().filter(|t| t.enabled) {
        changes.push(format!("Disable the scheduled task {}", t.path));
    }
    for s in &item.services {
        if s.start_type.eq_ignore_ascii_case("Disabled") {
            continue;
        }
        if s.start_type.eq_ignore_ascii_case(&s.planned_start_type) && !s.state.eq_ignore_ascii_case("Running") {
            continue;
        }
        let stop = if s.state.eq_ignore_ascii_case("Running") { "Stop the service" } else { "Set the service" };
        changes.push(format!(
            "{} \"{}\" and set it to {} (now {})",
            stop, s.display_name, s.planned_start_type, s.start_type
        ));
    }
    match item.kind {
        ItemKind::StoreApp => changes.push(format!("Uninstall {} for your account", item.name)),
        ItemKind::Win32App => changes.push(format!("Run {}'s own uninstaller", item.name)),
        _ => {}
    }
    item.planned_changes = changes;

    let auto_parts = !item.services.is_empty() || !item.startup.is_empty() || !item.tasks.is_empty();
    item.store_url = match (item.kind, item.package_family_name.as_deref()) {
        (ItemKind::StoreApp, Some(pfn)) => Some(format!("ms-windows-store://pdp/?PFN={}", pfn)),
        _ => None,
    };
    item.undo = match item.kind {
        ItemKind::StoreApp => "Reinstall it from the Microsoft Store. ASAO opens its Store page for you from Recent changes.".into(),
        ItemKind::Win32App => format!(
            "ASAO can't reinstall it automatically. {}{}",
            item.how_to_restore.clone().unwrap_or_else(|| "Download it again from the publisher.".into()),
            if auto_parts { " Its services and startup entries are restored if you undo before uninstalling." } else { "" }
        ),
        ItemKind::Service | ItemKind::StartupEntry => {
            "ASAO can undo this automatically from Recent changes.".into()
        }
    };
    // A Windows service row says so plainly.
    if item.category == Some(Category::OptionalWindowsService) && item.what_breaks.is_none() {
        item.what_breaks = Some("Windows features that use it may stop working.".into());
    }
}

#[cfg(test)]
pub mod tests_support {
    use super::{blank, BloatItem, ItemKind};

    pub fn blank_item(kind: ItemKind, name: &str) -> BloatItem {
        blank(format!("test:{}", name), kind, name.to_string())
    }
}
