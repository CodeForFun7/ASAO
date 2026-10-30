//! `ConfidenceScorer`: points for a folder that already passed every ownership check.

use super::history::InventoryHistory;
use super::inventory::InstalledApp;
use super::normalize::{
    is_generic_or_short, is_same_or_inside, match_names, normalize_name, normalize_publisher,
    strip_publisher, NameMatch,
};
use super::{Confidence, RootKind};

pub const HIGH_THRESHOLD: i32 = 12;
pub const MEDIUM_THRESHOLD: i32 = 7;

/// Matches an app in the history that is no longer installed.
pub const PTS_SNAPSHOT_MATCH: i32 = 8;
/// Program Files folder that was the removed app's install location.
pub const PTS_OLD_INSTALL_LOCATION: i32 = 8;
pub const PTS_EXACT_NAME: i32 = 6;
pub const PTS_FUZZY_NAME: i32 = 3;
pub const PTS_PARENT_IS_PUBLISHER: i32 = 3;
pub const PTS_DEPTH_ONE: i32 = 2;
pub const PTS_EMPTY_OR_LOGS: i32 = 2;
pub const PTS_GENERIC_OR_SHORT: i32 = -6;
/// Starting score for Program Files folders: install folders map cleanly to apps, so a
/// true orphan there starts at Medium. Every other signal still adds or subtracts.
pub const PROGRAM_FILES_BASE: i32 = 7;

#[derive(Debug, Clone)]
pub struct RemovedApp {
    pub key: String,
    pub display_name: String,
    name_norm: String,
    stripped_norm: Option<String>,
    publisher_norm: Option<String>,
    pub install_dirs: Vec<String>,
    pub removed_on_ms: Option<u64>,
}

/// Apps in the history that are gone now. Entries whose name matches something that is
/// installed again (under another key) are dropped, so a reinstall never scores.
pub fn removed_apps(history: &InventoryHistory, installed: &[InstalledApp]) -> Vec<RemovedApp> {
    let installed_norms: Vec<String> = installed
        .iter()
        .filter_map(|a| a.display_name.as_deref())
        .map(normalize_name)
        .filter(|n| !n.is_empty())
        .collect();
    history
        .removed()
        .filter_map(|e| {
            let name_norm = normalize_name(&e.display_name);
            if name_norm.is_empty() || installed_norms.iter().any(|n| n == &name_norm) {
                return None;
            }
            let publisher_norm = e
                .publisher
                .as_deref()
                .map(normalize_publisher)
                .filter(|p| !p.is_empty());
            let stripped_norm = publisher_norm
                .as_deref()
                .and_then(|p| strip_publisher(&name_norm, p));
            Some(RemovedApp {
                key: e.key.clone(),
                display_name: e.display_name.clone(),
                name_norm,
                stripped_norm,
                publisher_norm,
                install_dirs: e.install_dirs.clone(),
                removed_on_ms: e.removed_on_ms,
            })
        })
        .collect()
}

pub struct ScoreInput<'a> {
    pub folder_key: &'a str,
    pub folder_name: &'a str,
    pub parent_name: Option<&'a str>,
    pub depth: u8,
    pub root_kind: RootKind,
    pub empty_or_logs: bool,
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

        let path_match = input.root_kind.is_program_files()
            && app.install_dirs.iter().any(|d| {
                is_same_or_inside(input.folder_key, d) || is_same_or_inside(d, input.folder_key)
            });

        let mut name = NameMatch::None;
        for candidate in [Some(app.name_norm.as_str()), app.stripped_norm.as_deref()]
            .into_iter()
            .flatten()
        {
            let m = match_names(&folder_norm, candidate);
            if m == NameMatch::Exact || (m == NameMatch::Fuzzy && name == NameMatch::None) {
                name = m;
            }
        }

        if path_match || name != NameMatch::None {
            pts += PTS_SNAPSHOT_MATCH;
            why.push(format!("{} was uninstalled ({:+})", app.display_name, PTS_SNAPSHOT_MATCH));
        }
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

        // A parent-publisher hit alone is not a match to the app.
        if pts >= PTS_SNAPSHOT_MATCH && best.as_ref().map(|b| pts > b.0).unwrap_or(true) {
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
    if is_generic_or_short(&folder_norm) {
        total += PTS_GENERIC_OR_SHORT;
        reasons.push(format!("Generic or very short name ({:+})", PTS_GENERIC_OR_SHORT));
    }

    ScoreResult {
        score: total,
        confidence: confidence_for(total),
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
        ScoreInput { folder_key: key, folder_name: name, parent_name: parent, depth, root_kind: root, empty_or_logs: false, removed }
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
