//! Runs one scan, or re-checks one folder right before it is deleted.
//! The same `evaluate` decides both, so a scan and a pre-delete check can never disagree.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use super::exclusions::{ExclusionDecision, ExclusionList};
use super::history::{InventoryHistory, ObserveSummary};
use super::inventory::{collect_installed_apps, InstalledApp};
use super::normalize::{is_same_or_inside, normalize_name, path_key, stable_id};
use super::ownership::{OwnedReason, OwnershipChecker};
use super::references::collect_referenced_executables;
use super::roots::{
    find_microsoft_binary, folder_stats, is_in_onedrive, is_reparse_point, known_roots, list_dirs,
    onedrive_roots,
};
use super::evidence::collect_past_uninstalls;
use super::scoring::{removed_apps_with_evidence, score, RemovalSource, RemovedApp, ScoreInput};
use super::store_files::{IgnoreList, ResidualFiles};
use super::{
    now_ms, Confidence, ResidualCandidate, ResidualCategory, ResidualGroup, ResidualScanProgress,
    ResidualScanResult, ResidualScanStats, RootKind, ScanRoot,
};

const DAY_MS: u64 = 86_400_000;
/// Folders with more entries than this cannot be checked for recent files, so they are not flagged.
pub const MAX_WALK_ENTRIES: u64 = 250_000;

pub struct EngineContext {
    pub roots: Vec<ScanRoot>,
    pub installed: Vec<InstalledApp>,
    pub removed: Vec<RemovedApp>,
    /// Path keys of running and auto-started executables.
    pub exe_paths: Vec<String>,
    pub references_available: bool,
    pub exclusions: ExclusionList,
    pub min_age_days: u32,
    pub now_ms: u64,
    pub onedrive: Vec<String>,
    pub max_walk_entries: u64,
}

#[derive(Debug, Clone)]
pub enum Verdict {
    Candidate(ResidualCandidate),
    /// Not flagged itself; its children are evaluated one by one. The flag marks a vendor
    /// folder of installed apps: its children are flagged only when they match an app
    /// that was uninstalled, never as unknown orphans (they are likely shared vendor data).
    Container(String, bool),
    Owned(String),
    Excluded(String),
    Recent(String),
    Unverifiable(String),
}

pub struct Engine<'a> {
    ctx: &'a EngineContext,
    owner: OwnershipChecker,
}

pub fn categorize(root: RootKind, name: &str) -> ResidualCategory {
    if root.is_program_files() {
        return ResidualCategory::InstallRemnant;
    }
    let norm = normalize_name(name);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    if tokens.iter().any(|t| t.contains("cache")) {
        return ResidualCategory::Cache;
    }
    let log_words = ["log", "logs", "crashes", "crash", "crashdumps", "crashreports", "dumps"];
    if tokens.iter().any(|t| log_words.contains(t)) {
        return ResidualCategory::Logs;
    }
    ResidualCategory::AppData
}

impl<'a> Engine<'a> {
    pub fn new(ctx: &'a EngineContext) -> Self {
        let owner = OwnershipChecker::build(&ctx.installed, &ctx.exe_paths, &ctx.roots);
        Self { ctx, owner }
    }

    pub fn evaluate(
        &self,
        root: &ScanRoot,
        path: &Path,
        name: &str,
        depth: u8,
        parent_name: Option<&str>,
        under_vendor: bool,
    ) -> Verdict {
        let path_str = path.to_string_lossy().to_string();
        let key = path_key(&path_str);

        // Rule 5: exclusion list and ignore list
        match self.ctx.exclusions.check(&path_str, name) {
            ExclusionDecision::Never(reason) => return Verdict::Excluded(reason),
            ExclusionDecision::ChildrenOnly => {
                return if depth == 1 {
                    Verdict::Container(format!("{} holds separate apps", name), false)
                } else {
                    Verdict::Excluded(format!("{} is a shared folder", name))
                };
            }
            ExclusionDecision::Allowed => {}
        }

        // Rules 1 to 4
        if let Some(reason) = self.owner.owner_of(&path_str, name) {
            if depth == 1 && matches!(reason, OwnedReason::Publisher(_)) {
                return Verdict::Container(reason.describe(), true);
            }
            return Verdict::Owned(reason.describe());
        }

        // A vendor folder holding any installed product is never flagged as a whole.
        if depth == 1 {
            let mut scratch = Vec::new();
            let mut denied = 0u64;
            for (child_path, child_name) in list_dirs(path, &mut scratch, &mut denied) {
                if let Some(reason) = self.owner.owner_of(&child_path.to_string_lossy(), &child_name) {
                    return Verdict::Container(format!("Holds {}", reason.describe().to_lowercase()), true);
                }
            }
        }

        // Rule 6: anything modified recently means the folder is still in use
        let cutoff = if self.ctx.min_age_days > 0 {
            Some(self.ctx.now_ms.saturating_sub(self.ctx.min_age_days as u64 * DAY_MS))
        } else {
            None
        };
        let stats = folder_stats(path, cutoff, self.ctx.max_walk_entries);
        if stats.recent_found {
            return Verdict::Recent(format!(
                "A file inside changed in the last {} days",
                self.ctx.min_age_days
            ));
        }
        if stats.truncated {
            return Verdict::Unverifiable("Too many files to check safely".into());
        }

        // Windows and Microsoft components often have no uninstall entry. Never flag them.
        if let Some(file) = find_microsoft_binary(path, 60, 20_000) {
            return Verdict::Excluded(format!("Contains Microsoft components ({})", file));
        }

        let scored = score(&ScoreInput {
            folder_key: &key,
            folder_name: name,
            parent_name,
            depth,
            root_kind: root.kind,
            empty_or_logs: stats.is_empty_or_logs(),
            newest_modified_ms: stats.newest_modified_ms,
            now_ms: self.ctx.now_ms,
            removed: &self.ctx.removed,
        });

        if under_vendor && scored.matched.is_none() {
            return Verdict::Owned("Shared data of an installed publisher's apps".into());
        }
        if let Some(app) = &scored.matched {
            if self.ctx.exclusions.is_ignored_app(&app.key) {
                return Verdict::Excluded(format!("{} is on your ignore list", app.display_name));
            }
        }

        Verdict::Candidate(ResidualCandidate {
            id: stable_id(&key),
            path: path_str,
            name: name.to_string(),
            root_kind: root.kind,
            depth,
            category: categorize(root.kind, name),
            confidence: scored.confidence,
            score: scored.score,
            reasons: scored.reasons,
            matched_app_key: scored.matched.as_ref().map(|m| m.key.clone()),
            matched_app_name: scored.matched.as_ref().map(|m| m.display_name.clone()),
            removed_on_ms: scored.matched.as_ref().and_then(|m| m.removed_on_ms),
            last_run_ms: scored.matched.as_ref().and_then(|m| m.last_run_ms),
            newest_modified_ms: stats.newest_modified_ms,
            size_bytes: None,
            needs_admin: root.kind.needs_admin(),
            in_onedrive: is_in_onedrive(&key, &self.ctx.onedrive),
            preselected: scored.confidence == Confidence::High,
        })
    }

    pub fn scan(
        &self,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(&ResidualScanProgress),
    ) -> Result<ResidualScanResult, String> {
        let started = now_ms();
        let mut stats = ResidualScanStats {
            installed_apps: self.ctx.installed.len(),
            removed_apps_known: self.ctx.removed.len(),
            removed_from_evidence: self
                .ctx
                .removed
                .iter()
                .filter(|r| r.source != RemovalSource::History)
                .count(),
            references_available: self.ctx.references_available,
            ..Default::default()
        };
        let mut log: Vec<String> = Vec::new();

        // Top-level folders of every root. Each one (with its children, for vendor folders)
        // is one unit of work; units run in parallel because most of the time is disk I/O.
        let mut tasks: Vec<(usize, PathBuf, String)> = Vec::new();
        for (ri, root) in self.ctx.roots.iter().enumerate() {
            if !root.path.exists() {
                log.push(format!("Root not found: {}", root.path.display()));
                continue;
            }
            stats.roots_scanned += 1;
            for (p, n) in list_dirs(&root.path, &mut log, &mut stats.access_denied) {
                tasks.push((ri, p, n));
            }
        }

        struct TaskResult {
            index: usize,
            verdicts: Vec<Verdict>,
            log: Vec<String>,
            access_denied: u64,
        }

        let next = AtomicUsize::new(0);
        let checked = AtomicU64::new(0);
        let found = AtomicU64::new(0);
        let current = Mutex::new(String::new());
        let results: Mutex<Vec<TaskResult>> = Mutex::new(Vec::with_capacity(tasks.len()));
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(2, 8);
        let active = AtomicUsize::new(workers);

        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| {
                    loop {
                        if cancel.load(Ordering::SeqCst) {
                            break;
                        }
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some((ri, p1, n1)) = tasks.get(i) else { break };
                        let root = &self.ctx.roots[*ri];
                        if let Ok(mut c) = current.lock() {
                            *c = p1.to_string_lossy().to_string();
                        }
                        checked.fetch_add(1, Ordering::SeqCst);

                        let mut verdicts = Vec::new();
                        let mut wlog = Vec::new();
                        let mut denied = 0u64;
                        match self.evaluate(root, p1, n1, 1, None, false) {
                            Verdict::Container(_, vendor) => {
                                for (p2, n2) in list_dirs(p1, &mut wlog, &mut denied) {
                                    if cancel.load(Ordering::SeqCst) {
                                        break;
                                    }
                                    checked.fetch_add(1, Ordering::SeqCst);
                                    let v = self.evaluate(root, &p2, &n2, 2, Some(n1), vendor);
                                    if matches!(v, Verdict::Candidate(_)) {
                                        found.fetch_add(1, Ordering::SeqCst);
                                    }
                                    verdicts.push(v);
                                }
                                // The container itself counts as skipped.
                                verdicts.push(Verdict::Owned(String::new()));
                            }
                            other => {
                                if matches!(other, Verdict::Candidate(_)) {
                                    found.fetch_add(1, Ordering::SeqCst);
                                }
                                verdicts.push(other);
                            }
                        }
                        if let Ok(mut r) = results.lock() {
                            r.push(TaskResult {
                                index: i,
                                verdicts,
                                log: wlog,
                                access_denied: denied,
                            });
                        }
                    }
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }

            // Progress from this thread while the workers run.
            while active.load(Ordering::SeqCst) > 0 {
                let path = current.lock().map(|c| c.clone()).unwrap_or_default();
                progress(&ResidualScanProgress {
                    phase: "matching".into(),
                    current_path: path,
                    folders_checked: checked.load(Ordering::SeqCst),
                    candidates_found: found.load(Ordering::SeqCst),
                });
                std::thread::sleep(Duration::from_millis(100));
            }
        });

        if cancel.load(Ordering::SeqCst) {
            return Err("Scan cancelled.".into());
        }

        let mut results = results.into_inner().unwrap_or_default();
        results.sort_by_key(|r| r.index);
        let mut candidates: Vec<ResidualCandidate> = Vec::new();
        for r in results {
            log.extend(r.log);
            stats.access_denied += r.access_denied;
            for v in r.verdicts {
                match v {
                    Verdict::Candidate(c) => candidates.push(c),
                    Verdict::Container(..) | Verdict::Owned(_) | Verdict::Unverifiable(_) => {
                        stats.owned_skipped += 1
                    }
                    Verdict::Excluded(_) => stats.excluded_skipped += 1,
                    Verdict::Recent(_) => stats.recent_skipped += 1,
                }
            }
        }
        stats.folders_checked = checked.load(Ordering::SeqCst);

        Ok(ResidualScanResult {
            scan_id: started,
            started_ms: started,
            finished_ms: now_ms(),
            min_age_days: self.ctx.min_age_days,
            groups: group_candidates(candidates),
            stats,
            log,
        })
    }

    /// Fresh check right before deleting: the folder must still be a depth 1 or 2 folder
    /// of a scan root, must not be a link, and must still pass every ownership rule.
    pub fn verify_for_deletion(&self, path: &str) -> Result<ResidualCandidate, String> {
        if !self.ctx.references_available {
            return Err("Could not check services, scheduled tasks and startup entries".into());
        }
        let key = path_key(path);
        let (root, rel) = self
            .ctx
            .roots
            .iter()
            .find_map(|r| {
                let rk = path_key(&r.path.to_string_lossy());
                if key != rk && is_same_or_inside(&key, &rk) {
                    Some((r, key[rk.len()..].trim_start_matches('\\').to_string()))
                } else {
                    None
                }
            })
            .ok_or_else(|| "Not inside a folder this feature scans".to_string())?;
        let depth = rel.split('\\').filter(|s| !s.is_empty()).count();
        if depth == 0 || depth > 2 {
            return Err("Not a top-level app folder".into());
        }

        let p = Path::new(path);
        let meta = std::fs::symlink_metadata(p).map_err(|_| "The folder no longer exists".to_string())?;
        if !meta.is_dir() || is_reparse_point(&meta) {
            return Err("Not a regular folder".into());
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| "Invalid folder name".to_string())?;

        let mut parent_vendor = false;
        let parent_name = if depth == 2 {
            let parent = p.parent().ok_or_else(|| "Invalid folder".to_string())?;
            let parent_name = parent
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            // The parent must still be a folder whose children may be evaluated.
            match self.evaluate(root, parent, &parent_name, 1, None, false) {
                Verdict::Container(_, vendor) => parent_vendor = vendor,
                Verdict::Candidate(_) => {}
                Verdict::Owned(r) | Verdict::Excluded(r) | Verdict::Recent(r) | Verdict::Unverifiable(r) => {
                    return Err(format!("Parent folder is protected: {}", r));
                }
            }
            Some(parent_name)
        } else {
            None
        };

        match self.evaluate(root, p, &name, depth as u8, parent_name.as_deref(), parent_vendor) {
            Verdict::Candidate(c) => Ok(c),
            Verdict::Container(r, _)
            | Verdict::Owned(r)
            | Verdict::Excluded(r)
            | Verdict::Recent(r)
            | Verdict::Unverifiable(r) => Err(r),
        }
    }
}

pub fn group_candidates(candidates: Vec<ResidualCandidate>) -> Vec<ResidualGroup> {
    let mut groups: HashMap<String, ResidualGroup> = HashMap::new();
    for c in candidates {
        let (key, name, unknown) = match (&c.matched_app_key, &c.matched_app_name) {
            (Some(k), Some(n)) => (k.clone(), n.clone(), false),
            _ => ("unknown".to_string(), "Unknown leftovers".to_string(), true),
        };
        let g = groups.entry(key.clone()).or_insert_with(|| ResidualGroup {
            key,
            app_name: name,
            is_unknown: unknown,
            confidence: Confidence::Low,
            removed_on_ms: None,
            last_run_ms: None,
            folders: Vec::new(),
        });
        if c.confidence > g.confidence {
            g.confidence = c.confidence;
        }
        if g.removed_on_ms.is_none() {
            g.removed_on_ms = c.removed_on_ms;
        }
        if g.last_run_ms.is_none() {
            g.last_run_ms = c.last_run_ms;
        }
        g.folders.push(c);
    }
    let mut out: Vec<ResidualGroup> = groups.into_values().collect();
    for g in &mut out {
        g.folders.sort_by(|a, b| b.score.cmp(&a.score).then(a.path.cmp(&b.path)));
    }
    out.sort_by(|a, b| {
        a.is_unknown
            .cmp(&b.is_unknown)
            .then(b.confidence.cmp(&a.confidence))
            .then(a.app_name.to_lowercase().cmp(&b.app_name.to_lowercase()))
    });
    out
}

/// What the live context should include.
pub struct LiveOptions {
    pub min_age_days: u32,
    /// Path keys of running executables (from ASAO's process monitor).
    pub running_exes: Vec<String>,
    /// Folders that must never be flagged (ASAO's own data, its install folder).
    pub protected_paths: Vec<String>,
    /// Record the current inventory in the app history before matching.
    pub update_history: bool,
    /// Limit to these root kinds (the administrator helper only handles machine roots).
    pub only_roots: Option<Vec<RootKind>>,
}

pub struct LiveContext {
    pub ctx: EngineContext,
    pub observe: Option<ObserveSummary>,
}

/// Builds a context from this machine: inventory, history, references, roots, ignore list.
pub fn build_live_context(files: &ResidualFiles, opts: LiveOptions) -> Result<LiveContext, String> {
    let now = now_ms();
    let inventory = collect_installed_apps();
    if inventory.failed_classes.contains(&"registry") {
        return Err("Could not read the list of installed apps from the registry.".into());
    }

    let mut history = InventoryHistory::load(&files.history());
    let observe = if opts.update_history {
        let summary = history.observe(&inventory.apps, &inventory.failed_classes, now);
        history.save(&files.history())?;
        Some(summary)
    } else {
        None
    };
    // Evidence of apps uninstalled before ASAO kept its history.
    let evidence = collect_past_uninstalls();
    let removed = removed_apps_with_evidence(&history, &evidence, &inventory.apps);

    let references = collect_referenced_executables();
    let mut exe_paths = opts.running_exes;
    if let Some(refs) = &references {
        exe_paths.extend(refs.iter().cloned());
    }
    exe_paths.sort();
    exe_paths.dedup();

    let mut roots = known_roots();
    if let Some(only) = &opts.only_roots {
        roots.retain(|r| only.contains(&r.kind));
    }

    let mut protected = opts.protected_paths;
    if let Some(parent) = files.dir.parent() {
        protected.push(parent.to_string_lossy().to_string());
    }
    if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        protected.push(dir.to_string_lossy().to_string());
    }
    // Without Store data, Store app folders cannot be told apart: protect them all.
    if inventory.failed_classes.contains(&"store") {
        if let Some(local) = roots.iter().find(|r| r.kind == RootKind::LocalAppData) {
            protected.push(local.path.join("Packages").to_string_lossy().to_string());
        }
    }

    let ignore = IgnoreList::load(&files.ignore_list());
    let exclusions = ExclusionList::new(protected).with_ignores(&ignore.folder_paths(), &ignore.app_keys());

    Ok(LiveContext {
        ctx: EngineContext {
            roots,
            installed: inventory.apps,
            removed,
            exe_paths,
            references_available: references.is_some(),
            exclusions,
            min_age_days: super::sanitize_min_age_days(opts.min_age_days),
            now_ms: now,
            onedrive: onedrive_roots(),
            max_walk_entries: MAX_WALK_ENTRIES,
        },
        observe,
    })
}

/// Records the current inventory in the app history. Runs quietly at every launch.
pub fn refresh_history(files: &ResidualFiles) -> Result<ObserveSummary, String> {
    let inventory = collect_installed_apps();
    if inventory.failed_classes.contains(&"registry") {
        return Err("Registry inventory unavailable".into());
    }
    let mut history = InventoryHistory::load(&files.history());
    let summary = history.observe(&inventory.apps, &inventory.failed_classes, now_ms());
    history.save(&files.history())?;
    Ok(summary)
}
