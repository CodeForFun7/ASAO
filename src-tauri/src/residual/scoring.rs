//! `ConfidenceScorer`: points for a folder that already passed every ownership check.

use super::evidence::{EvidenceSource, PastUninstall};
use super::history::InventoryHistory;
use super::inventory::InstalledApp;
use super::normalize::{
    folder_name_variants, is_generic_or_short, is_same_or_inside, match_names, match_names_for_scoring,
    normalize_name, normalize_publisher, strip_publisher, token_contains, NameMatch,
};
use super::{Confidence, RootKind};

pub const HIGH_THRESHOLD: i32 = 12;
pub const MEDIUM_THRESHOLD: i32 = 7;

/// Matches an app in the history that is no longer installed.
pub const PTS_SNAPSHOT_MATCH: i32 = 8;
/// Matches an app the Windows Installer log says was uninstalled. As strong as the history.
pub const PTS_MSI_LOG_MATCH: i32 = 8;
/// Matches a program that was run on this PC (UserAssist) and whose file is gone now.
/// A little weaker: the program may have been moved or was portable.
pub const PTS_USERASSIST_MATCH: i32 = 6;
/// Folder that was the removed app's install location.
pub const PTS_OLD_INSTALL_LOCATION: i32 = 8;
pub const PTS_EXACT_NAME: i32 = 6;
pub const PTS_FUZZY_NAME: i32 = 3;
pub const PTS_PARENT_IS_PUBLISHER: i32 = 3;
pub const PTS_DEPTH_ONE: i32 = 2;
pub const PTS_EMPTY_OR_LOGS: i32 = 2;
/// Nothing inside the folder changed for over a year.
pub const PTS_DORMANT_YEAR: i32 = 3;
pub const PTS_GENERIC_OR_SHORT: i32 = -6;
/// Starting score for Program Files folders: install folders map cleanly to apps, so a
/// true orphan there starts at Medium. Every other signal still adds or subtracts.
pub const PROGRAM_FILES_BASE: i32 = 7;

const YEAR_MS: u64 = 365 * 86_400_000;

/// Where the knowledge that an app was uninstalled comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovalSource {
    /// ASAO saw it installed, then gone.
    History,
    /// Windows Installer logged its removal.
    MsiLog,
    /// It was run on this PC and its program file no longer exists.
    UserAssist,
}

impl RemovalSource {
    fn points(&self) -> i32 {
        match self {
            RemovalSource::History => PTS_SNAPSHOT_MATCH,
            RemovalSource::MsiLog => PTS_MSI_LOG_MATCH,
            RemovalSource::UserAssist => PTS_USERASSIST_MATCH,
        }
    }

    fn reason(&self, name: &str) -> String {
        match self {
            RemovalSource::History => format!("{} was uninstalled ({:+})", name, self.points()),
            RemovalSource::MsiLog => format!(
                "Windows Installer log shows {} was uninstalled ({:+})",
                name,
                self.points()
            ),
            RemovalSource::UserAssist => format!(
                "{} was run on this PC and its program is gone ({:+})",
                name,
                self.points()
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemovedApp {
    pub key: String,
    pub display_name: String,
    name_norm: String,
    stripped_norm: Option<String>,
    /// Other names to match: an exe name from UserAssist, for example.
    alt_norms: Vec<String>,
    publisher_norm: Option<String>,
    pub install_dirs: Vec<String>,
    pub removed_on_ms: Option<u64>,
    pub last_run_ms: Option<u64>,
    pub source: RemovalSource,
}

fn removed_app(
    key: String,
    display_name: &str,
    publisher: Option<&str>,
    alt_names: &[String],
    install_dirs: Vec<String>,
    removed_on_ms: Option<u64>,
    last_run_ms: Option<u64>,
    source: RemovalSource,
) -> Option<RemovedApp> {
    let name_norm = normalize_name(display_name);
    if name_norm.is_empty() {
        return None;
    }
    let publisher_norm = publisher.map(normalize_publisher).filter(|p| !p.is_empty());
    let stripped_norm = publisher_norm
        .as_deref()
        .and_then(|p| strip_publisher(&name_norm, p));
    let alt_norms = alt_names
        .iter()
        .map(|a| normalize_name(a))
        .filter(|a| !a.is_empty() && a != &name_norm)
        .collect();
    Some(RemovedApp {
        key,
        display_name: display_name.to_string(),
        name_norm,
        stripped_norm,
        alt_norms,
        publisher_norm,
        install_dirs,
        removed_on_ms,
        last_run_ms,
        source,
    })
}

/// Names of what is installed now, used to drop stale removal evidence.
struct InstalledNames {
    names: Vec<String>,
    dirs: Vec<String>,
}

impl InstalledNames {
    fn new(installed: &[InstalledApp]) -> Self {
        let mut names = Vec::new();
        let mut dirs = Vec::new();
        for a in installed {
            if let Some(n) = a.display_name.as_deref() {
                let norm = normalize_name(n);
                if !norm.is_empty() {
                    if let Some(p) = a.publisher.as_deref().map(normalize_publisher) {
                        if let Some(s) = strip_publisher(&norm, &p) {
                            names.push(s);
                        }
                    }
                    names.push(norm);
                }
            }
            // Store apps: "SpotifyAB.SpotifyMusic" counts as "spotifyab" and "spotifymusic"
            if let Some(pkg) = a.package_name.as_deref() {
                for part in pkg.split(['.', '_']) {
                    let p = part.to_lowercase();
                    if p.chars().count() >= 4 {
                        names.push(p);
                    }
                }
            }
            dirs.extend(a.owned_dirs().into_iter().filter(|d| d.len() > 3));
        }
        Self { names, dirs }
    }

    /// True when a name, or one of the folders, still belongs to something installed.
    fn claims(&self, name_norm: &str, dirs: &[String]) -> bool {
        self.names.iter().any(|n| {
            match_names(name_norm, n) != NameMatch::None || token_contains(n, name_norm)
        }) || dirs.iter().any(|d| {
            self.dirs
                .iter()
                .any(|i| is_same_or_inside(d, i) || is_same_or_inside(i, d))
        })
    }
}

/// Apps in the history that are gone now. Entries whose name matches something that is
/// installed again (under another key) are dropped, so a reinstall never scores.
pub fn removed_apps(history: &InventoryHistory, installed: &[InstalledApp]) -> Vec<RemovedApp> {
    removed_apps_with_evidence(history, &[], installed)
}

/// History entries first, then evidence of older uninstalls (Windows Installer log, then
/// UserAssist). Evidence for a name already known, or for anything installed now, is dropped.
pub fn removed_apps_with_evidence(
    history: &InventoryHistory,
    evidence: &[PastUninstall],
    installed: &[InstalledApp],
) -> Vec<RemovedApp> {
    let installed_names = InstalledNames::new(installed);
    let exact_installed: Vec<String> = installed
        .iter()
        .filter_map(|a| a.display_name.as_deref())
        .map(normalize_name)
        .filter(|n| !n.is_empty())
        .collect();

    let mut out: Vec<RemovedApp> = history
        .removed()
        .filter(|e| {
            let n = normalize_name(&e.display_name);
            !n.is_empty() && !exact_installed.iter().any(|i| i == &n)
        })
        .filter_map(|e| {
            removed_app(
                e.key.clone(),
                &e.display_name,
                e.publisher.as_deref(),
                &[],
                e.install_dirs.clone(),
                e.removed_on_ms,
                None,
                RemovalSource::History,
            )
        })
        .collect();

    let mut ordered: Vec<&PastUninstall> = evidence.iter().collect();
    // Installer log before UserAssist, so the stronger source wins a tie on name.
    ordered.sort_by_key(|e| match e.source {
        EvidenceSource::MsiLog => 0,
        EvidenceSource::UserAssist => 1,
    });
    for e in ordered {
        let name_norm = normalize_name(&e.display_name);
        if name_norm.is_empty() {
            continue;
        }
        if let Some(existing) = out.iter_mut().find(|r| r.name_norm == name_norm) {
            // Same app seen again (several exes, several games under one vendor folder):
            // keep its extra names and folders. Never widen a history entry.
            if existing.source != RemovalSource::History {
                for alt in &e.alt_names {
                    let n = normalize_name(alt);
                    if !n.is_empty() && n != existing.name_norm && !existing.alt_norms.contains(&n)
                        && !installed_names.claims(&n, &[])
                    {
                        existing.alt_norms.push(n);
                    }
                }
                for d in &e.install_dirs {
                    if !existing.install_dirs.contains(d) {
                        existing.install_dirs.push(d.clone());
                    }
                }
                if existing.last_run_ms < e.last_run_ms {
                    existing.last_run_ms = e.last_run_ms;
                }
            }
            continue;
        }
        let alt_claimed = e
            .alt_names
            .iter()
            .map(|a| normalize_name(a))
            .any(|a| !a.is_empty() && installed_names.claims(&a, &[]));
        if alt_claimed || installed_names.claims(&name_norm, &e.install_dirs) {
            continue;
        }
        let source = match e.source {
            EvidenceSource::MsiLog => RemovalSource::MsiLog,
            EvidenceSource::UserAssist => RemovalSource::UserAssist,
        };
        if let Some(app) = removed_app(
            format!("evidence|{}", name_norm),
            &e.display_name,
            e.publisher.as_deref(),
            &e.alt_names,
            e.install_dirs.clone(),
            e.removed_on_ms,
            e.last_run_ms,
            source,
        ) {
            out.push(app);
        }
    }
    out
}

pub struct ScoreInput<'a> {
    pub folder_key: &'a str,
    pub folder_name: &'a str,
    pub parent_name: Option<&'a str>,
    pub depth: u8,
    pub root_kind: RootKind,
    pub empty_or_logs: bool,
    pub newest_modified_ms: Option<u64>,
    pub now_ms: u64,
    pub removed: &'a [RemovedApp],
}

#[derive(Debug, Clone)]
pub struct ScoreResult {
    pub score: i32,
    pub confidence: Confidence,
    pub matched: Option<RemovedApp>,
    pub reasons: Vec<String>,
}

pub fn confidence_for(score: i32) -> Confidence {
    if score >= HIGH_THRESHOLD {
        Confidence::High
    } else if score >= MEDIUM_THRESHOLD {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

pub fn score(input: &ScoreInput) -> ScoreResult {
    let folder_norm = normalize_name(input.folder_name);
    let mut folder_names = vec![folder_norm.clone()];
    folder_names.extend(folder_name_variants(input.folder_name));
    let parent_norm = input.parent_name.map(normalize_publisher);
    let mut reasons = Vec::new();
    let mut total = 0;

    if input.root_kind.is_program_files() {
        total += PROGRAM_FILES_BASE;
        reasons.push(format!("In {} ({:+})", input.root_kind.label(), PROGRAM_FILES_BASE));
    }

    // Best-matching removed app
    let mut best: Option<(i32, Vec<String>, &RemovedApp)> = None;
    for app in input.removed {
        let mut pts = 0;
        let mut why = Vec::new();

        // The app's former install folder, in any root (Program Files, or per-user installs
        // under AppData\Local\Programs).
        let path_match = app.install_dirs.iter().any(|d| {
            is_same_or_inside(input.folder_key, d) || is_same_or_inside(d, input.folder_key)
        });

        let mut name = NameMatch::None;
        let candidates = std::iter::once(app.name_norm.as_str())
            .chain(app.stripped_norm.as_deref())
            .chain(app.alt_norms.iter().map(|s| s.as_str()));
        for candidate in candidates {
            for folder in &folder_names {
                let m = match_names_for_scoring(folder, candidate);
                if m == NameMatch::Exact || (m == NameMatch::Fuzzy && name == NameMatch::None) {
                    name = m;
                }
            }
        }

        // A parent-publisher hit alone is not a match to the app.
        if !path_match && name == NameMatch::None {
            continue;
        }
        pts += app.source.points();
        why.push(app.source.reason(&app.display_name));
        if path_match {
            pts += PTS_OLD_INSTALL_LOCATION;
            why.push(format!("Was its install folder ({:+})", PTS_OLD_INSTALL_LOCATION));
        }
        match name {
            NameMatch::Exact => {
                pts += PTS_EXACT_NAME;
                why.push(format!("Name matches {} ({:+})", app.display_name, PTS_EXACT_NAME));
            }
            NameMatch::Fuzzy => {
                pts += PTS_FUZZY_NAME;
                why.push(format!("Name is close to {} ({:+})", app.display_name, PTS_FUZZY_NAME));
            }
            NameMatch::None => {}
        }
        if let (Some(parent), Some(publ)) = (parent_norm.as_deref(), app.publisher_norm.as_deref()) {
            if input.depth == 2 && match_names(parent, publ) == NameMatch::Exact {
                pts += PTS_PARENT_IS_PUBLISHER;
                why.push(format!("Inside its publisher folder ({:+})", PTS_PARENT_IS_PUBLISHER));
            }
        }

        if best.as_ref().map(|b| pts > b.0).unwrap_or(true) {
            best = Some((pts, why, app));
        }
    }

    let matched = best.as_ref().map(|b| b.2.clone());
    if let Some((pts, why, _)) = best {
        total += pts;
        reasons.extend(why);
    } else {
        reasons.push("No uninstalled app matches this folder".into());
    }

    if input.depth == 1 {
        total += PTS_DEPTH_ONE;
        reasons.push(format!("Directly in {} ({:+})", input.root_kind.label(), PTS_DEPTH_ONE));
    }
    if input.empty_or_logs {
        total += PTS_EMPTY_OR_LOGS;
        reasons.push(format!("Empty, or only logs and caches ({:+})", PTS_EMPTY_OR_LOGS));
    }
    if let Some(newest) = input.newest_modified_ms {
        if newest > 0 && input.now_ms.saturating_sub(newest) >= YEAR_MS {
            total += PTS_DORMANT_YEAR;
            reasons.push(format!("Nothing changed in over a year ({:+})", PTS_DORMANT_YEAR));
        }
    }
    if is_generic_or_short(&folder_norm) {
        total += PTS_GENERIC_OR_SHORT;
        reasons.push(format!("Generic or very short name ({:+})", PTS_GENERIC_OR_SHORT));
    }

    // Without an uninstalled app to point at, a folder is never pre-ticked.
    let mut confidence = confidence_for(total);
    if matched.is_none() && confidence == Confidence::High {
        confidence = Confidence::Medium;
        reasons.push("Capped at Medium: no uninstalled app matches it".into());
    }

    ScoreResult {
        score: total,
        confidence,
        matched,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::residual::history::AppHistoryEntry;

    fn history_with(name: &str, publisher: &str, dirs: &[&str]) -> InventoryHistory {
        InventoryHistory {
            version: 1,
            entries: vec![AppHistoryEntry {
                key: format!("{}|{}", normalize_name(name), normalize_publisher(publisher)),
                display_name: name.into(),
                publisher: Some(publisher.into()),
                install_dirs: dirs.iter().map(|s| s.to_string()).collect(),
                package_family_name: None,
                source_class: "registry".into(),
                first_seen_ms: 1,
                last_seen_ms: 2,
                removed_on_ms: Some(3),
            }],
            last_updated_ms: 3,
            observations: 2,
        }
    }

    fn input<'a>(key: &'a str, name: &'a str, parent: Option<&'a str>, depth: u8, root: RootKind, removed: &'a [RemovedApp]) -> ScoreInput<'a> {
        ScoreInput { folder_key: key, folder_name: name, parent_name: parent, depth, root_kind: root, empty_or_logs: false, newest_modified_ms: None, now_ms: 0, removed }
    }

    #[test]
    fn exact_match_to_removed_app_is_high() {
        let removed = removed_apps(&history_with("Audacity 3.4", "Audacity Team", &[]), &[]);
        let r = score(&input(r"c:\u\appdata\roaming\audacity", "audacity", None, 1, RootKind::RoamingAppData, &removed));
        assert_eq!(r.score, 8 + 6 + 2);
        assert_eq!(r.confidence, Confidence::High);
        assert_eq!(r.matched.unwrap().display_name, "Audacity 3.4");
    }

    #[test]
    fn program_files_old_install_location_is_high() {
        let removed = removed_apps(&history_with("Contoso Paint", "Contoso", &[r"c:\program files\contoso paint"]), &[]);
        let r = score(&input(r"c:\program files\contoso paint", "Contoso Paint", None, 1, RootKind::ProgramFiles, &removed));
        assert_eq!(r.score, 7 + 8 + 8 + 6 + 2);
        assert_eq!(r.confidence, Confidence::High);
    }

    #[test]
    fn program_files_orphan_starts_at_medium_and_signals_apply() {
        let none: Vec<RemovedApp> = vec![];
        let depth2 = score(&input(r"c:\program files\vendor\tool", "Tool Suite", Some("Vendor"), 2, RootKind::ProgramFiles, &none));
        assert_eq!(depth2.score, 7);
        assert_eq!(depth2.confidence, Confidence::Medium);
        let generic = score(&input(r"c:\program files\data", "Data", None, 1, RootKind::ProgramFiles, &none));
        assert_eq!(generic.score, 7 + 2 - 6);
        assert_eq!(generic.confidence, Confidence::Low);
    }

    #[test]
    fn pure_orphan_outside_program_files_is_low() {
        let none: Vec<RemovedApp> = vec![];
        let mut i = input(r"c:\u\appdata\roaming\mystery", "Mystery Folder", None, 1, RootKind::RoamingAppData, &none);
        i.empty_or_logs = true;
        let r = score(&i);
        assert_eq!(r.score, 4);
        assert_eq!(r.confidence, Confidence::Low);
        assert!(r.matched.is_none());
    }

    #[test]
    fn parent_publisher_and_fuzzy() {
        let removed = removed_apps(&history_with("Contoso Painter", "Contoso", &[]), &[]);
        let r = score(&input(r"c:\u\appdata\roaming\contoso\painter", "Painter", Some("Contoso"), 2, RootKind::RoamingAppData, &removed));
        assert_eq!(r.score, 8 + 6 + 3);
        let fuzzy = score(&input(r"c:\u\appdata\roaming\paintr", "Paintr", None, 2, RootKind::RoamingAppData, &removed));
        assert_eq!(fuzzy.score, 8 + 3);
        assert_eq!(fuzzy.confidence, Confidence::Medium);
    }

    fn evidence(source: EvidenceSource, name: &str, alt: &[&str], dirs: &[&str]) -> PastUninstall {
        PastUninstall {
            source,
            display_name: name.into(),
            publisher: None,
            alt_names: alt.iter().map(|s| s.to_string()).collect(),
            install_dirs: dirs.iter().map(|s| s.to_string()).collect(),
            removed_on_ms: Some(10),
            last_run_ms: Some(20),
        }
    }

    #[test]
    fn userassist_evidence_scores_high_on_exact_match() {
        let ev = vec![evidence(EvidenceSource::UserAssist, "Fooapp", &["FooTool"], &[r"c:\program files\fooapp"])];
        let removed = removed_apps_with_evidence(&InventoryHistory::default(), &ev, &[]);
        assert_eq!(removed.len(), 1);
        let r = score(&input(r"c:\u\appdata\roaming\fooapp", "Fooapp", None, 1, RootKind::RoamingAppData, &removed));
        assert_eq!(r.score, 6 + 6 + 2);
        assert_eq!(r.confidence, Confidence::High);
        // The exe name also matches
        let alt = score(&input(r"c:\u\appdata\local\footool", "FooTool", None, 1, RootKind::LocalAppData, &removed));
        assert_eq!(alt.score, 6 + 6 + 2);
        // At depth 2 without other signals a UserAssist match is still High
        let deep = score(&input(r"c:\u\appdata\local\vendor\fooapp", "Fooapp", Some("Vendor"), 2, RootKind::LocalAppData, &removed));
        assert_eq!(deep.score, 12);
    }

    #[test]
    fn msi_evidence_counts_like_history_and_history_wins_ties() {
        let ev = vec![
            evidence(EvidenceSource::MsiLog, "Contoso Studio 2.1", &[], &[]),
            evidence(EvidenceSource::UserAssist, "Audacity", &[], &[]),
        ];
        let h = history_with("Audacity", "Audacity Team", &[]);
        let removed = removed_apps_with_evidence(&h, &ev, &[]);
        assert_eq!(removed.len(), 2);
        assert_eq!(removed[0].source, RemovalSource::History);
        assert_eq!(removed[1].source, RemovalSource::MsiLog);
        let r = score(&input(r"c:\u\appdata\roaming\contoso studio", "Contoso Studio", None, 1, RootKind::RoamingAppData, &removed));
        assert_eq!(r.score, 8 + 6 + 2);
    }

    #[test]
    fn evidence_for_installed_apps_is_dropped() {
        let installed = vec![crate::residual::inventory::InstalledApp {
            id: "a".into(),
            display_name: Some("Google Chrome".into()),
            publisher: Some("Google LLC".into()),
            install_location: Some(r"C:\Program Files\Google\Chrome\Application".into()),
            uninstall_string: None,
            display_icon: None,
            source: crate::residual::inventory::AppSource::RegistryMachine,
            hidden: false,
            package_family_name: None,
            package_name: None,
        }];
        let ev = vec![
            // Old version folder of an app that is still installed
            evidence(EvidenceSource::UserAssist, "Chrome", &[], &[r"c:\program files\google\chrome\application\119.0"]),
            // Exe name claimed by an installed app
            evidence(EvidenceSource::UserAssist, "Something", &["Chrome"], &[]),
        ];
        assert!(removed_apps_with_evidence(&InventoryHistory::default(), &ev, &installed).is_empty());
    }

    #[test]
    fn dormant_folders_gain_points() {
        let none: Vec<RemovedApp> = vec![];
        let now = 10 * YEAR_MS;
        let mut i = input(r"c:\u\appdata\roaming\olddata", "Old Stuff", None, 1, RootKind::RoamingAppData, &none);
        i.empty_or_logs = true;
        i.now_ms = now;
        i.newest_modified_ms = Some(now - 2 * YEAR_MS);
        let r = score(&i);
        assert_eq!(r.score, 2 + 2 + 3);
        assert_eq!(r.confidence, Confidence::Medium);
        i.newest_modified_ms = Some(now - YEAR_MS / 2);
        assert_eq!(score(&i).score, 4);
    }

    #[test]
    fn unmatched_folders_are_never_high() {
        let none: Vec<RemovedApp> = vec![];
        let now = 10 * YEAR_MS;
        let mut i = input(r"c:\program files\oldvendor", "Old Vendor", None, 1, RootKind::ProgramFiles, &none);
        i.empty_or_logs = true;
        i.now_ms = now;
        i.newest_modified_ms = Some(now - 3 * YEAR_MS);
        let r = score(&i);
        assert_eq!(r.score, 7 + 2 + 2 + 3);
        assert_eq!(r.confidence, Confidence::Medium);
    }

    #[test]
    fn reinstalled_app_never_scores() {
        let h = history_with("Audacity", "Audacity Team", &[]);
        let installed = vec![crate::residual::inventory::InstalledApp {
            id: "a".into(),
            display_name: Some("Audacity".into()),
            publisher: None,
            install_location: None,
            uninstall_string: None,
            display_icon: None,
            source: crate::residual::inventory::AppSource::RegistryUser,
            hidden: false,
            package_family_name: None,
            package_name: None,
        }];
        assert!(removed_apps(&h, &installed).is_empty());
    }
}
