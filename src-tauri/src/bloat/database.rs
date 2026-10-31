//! `BloatwareDatabase`: loads and validates `bloatware-db.json`, and evaluates its match
//! rules and conditions. The file ships inside ASAO; a valid copy in the app data folder
//! (`bloat\bloatware-db.json`) replaces it, so the list can be updated without a rebuild.

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;

use super::{Category, Recommendation, Safety};

/// The database that ships with ASAO.
pub const BUILT_IN: &str = include_str!("../../resources/bloatware-db.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceAction {
    Manual,
    Disabled,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbMatch {
    #[serde(default)]
    pub package_family: Vec<String>,
    #[serde(default)]
    pub win32_name: Vec<String>,
    #[serde(default)]
    pub win32_publisher: Vec<String>,
    #[serde(default)]
    pub service: Vec<String>,
    #[serde(default)]
    pub startup: Vec<String>,
    #[serde(default)]
    pub task: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbCondition {
    /// "games-installed" or "org-managed".
    pub when: String,
    pub safety: Option<Safety>,
    pub recommendation: Option<Recommendation>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbEntry {
    pub id: String,
    #[serde(rename = "match")]
    pub matcher: DbMatch,
    pub display_name: String,
    pub category: Category,
    pub what_it_does: String,
    pub safety: Safety,
    pub safety_reason: String,
    pub what_breaks: String,
    pub how_to_restore: String,
    pub recommendation: Recommendation,
    #[serde(default)]
    pub conditions: Vec<DbCondition>,
    #[serde(default = "default_action")]
    pub service_action: ServiceAction,
    pub source: String,
    /// Ticked by default (Win11Debloat's SelectedByDefault). ASAO's own entries never are.
    #[serde(default)]
    pub selected_by_default: bool,
}

fn default_action() -> ServiceAction {
    ServiceAction::Manual
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DbFile {
    version: u32,
    updated: String,
    entries: Vec<DbEntry>,
}

pub const KNOWN_CONDITIONS: &[&str] = &["games-installed", "org-managed"];

/// Facts about this PC that conditions depend on.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConditionFacts {
    pub games_installed: bool,
    pub org_managed: bool,
}

/// An entry's verdict after its conditions.
#[derive(Debug, Clone)]
pub struct Rated {
    pub safety: Safety,
    pub recommendation: Recommendation,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BloatwareDatabase {
    pub version: u32,
    pub updated: String,
    pub entries: Vec<DbEntry>,
}

/// Case-insensitive match with `*` as "any text".
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] != '*' && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn any_match(patterns: &[String], text: &str) -> bool {
    !text.is_empty() && patterns.iter().any(|p| wildcard_match(p, text))
}

impl BloatwareDatabase {
    pub fn parse(json: &str) -> Result<Self, String> {
        let file: DbFile = serde_json::from_str(json).map_err(|e| format!("Not a valid database file: {}", e))?;
        let db = Self {
            version: file.version,
            updated: file.updated,
            entries: file.entries,
        };
        db.validate()?;
        Ok(db)
    }

    pub fn built_in() -> Self {
        Self::parse(BUILT_IN).expect("the built-in bloatware database is valid")
    }

    /// The override file when it exists and is valid, otherwise the built-in list.
    /// Returns the database, where it came from, and a problem with the override, if any.
    pub fn load(override_path: &Path) -> (Self, String, Option<String>) {
        if override_path.exists() {
            match std::fs::read_to_string(override_path)
                .map_err(|e| e.to_string())
                .and_then(|s| Self::parse(s.trim_start_matches('\u{feff}')))
            {
                Ok(db) => return (db, override_path.to_string_lossy().to_string(), None),
                Err(e) => {
                    return (
                        Self::built_in(),
                        "Built in".into(),
                        Some(format!("Ignored {}: {}", override_path.display(), e)),
                    )
                }
            }
        }
        (Self::built_in(), "Built in".into(), None)
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        for e in &self.entries {
            let at = |what: &str| format!("Entry '{}': {}", e.id, what);
            if e.id.trim().is_empty() {
                return Err("An entry has no id".into());
            }
            if !ids.insert(e.id.to_lowercase()) {
                return Err(at("duplicate id"));
            }
            for (field, value) in [
                ("displayName", &e.display_name),
                ("whatItDoes", &e.what_it_does),
                ("safetyReason", &e.safety_reason),
                ("whatBreaks", &e.what_breaks),
                ("howToRestore", &e.how_to_restore),
                ("source", &e.source),
            ] {
                if value.trim().is_empty() {
                    return Err(at(&format!("{} is empty", field)));
                }
            }
            let m = &e.matcher;
            let patterns = m.package_family.len()
                + m.win32_name.len()
                + m.win32_publisher.len()
                + m.service.len()
                + m.startup.len()
                + m.task.len();
            if patterns == 0 {
                return Err(at("has no match rule"));
            }
            if matches!(e.safety, Safety::Protected | Safety::NotRated) {
                return Err(at("safety must be safe, usually-safe, caution or not-recommended"));
            }
            let consistent = match e.recommendation {
                Recommendation::Remove => e.safety == Safety::Safe,
                Recommendation::Optional => matches!(e.safety, Safety::UsuallySafe | Safety::Caution),
                Recommendation::Keep => matches!(e.safety, Safety::Caution | Safety::NotRecommended),
            };
            if !consistent {
                return Err(at("recommendation does not fit its safety level"));
            }
            for c in &e.conditions {
                if !KNOWN_CONDITIONS.contains(&c.when.as_str()) {
                    return Err(at(&format!("unknown condition '{}'", c.when)));
                }
            }
        }
        Ok(())
    }

    /// The bundled Win11Debloat apps plus ASAO's built-in entries (no override files).
    pub fn built_in_combined() -> Self {
        let mut db = Self::built_in();
        let mut entries = super::w11d::Win11Debloat::built_in().db_entries();
        entries.extend(db.entries);
        db.entries = entries;
        db
    }

    /// Win11Debloat's apps first (they decide for every Store app they list), then ASAO's own
    /// entries. Returns the combined database, the Win11Debloat data, and any problems.
    pub fn load_combined(bloat_dir: &Path) -> (Self, super::w11d::Win11Debloat, String, Vec<String>) {
        let mut problems = Vec::new();
        let (mut db, source, p1) = Self::load(&bloat_dir.join("bloatware-db.json"));
        let (w, p2) = super::w11d::Win11Debloat::load(&bloat_dir.join("win11debloat"));
        problems.extend(p1);
        problems.extend(p2);
        let mut entries = w.db_entries();
        let ids: HashSet<String> = entries.iter().map(|e| e.id.to_lowercase()).collect();
        entries.extend(db.entries.into_iter().filter(|e| !ids.contains(&e.id.to_lowercase())));
        db.entries = entries;
        (db, w, source, problems)
    }

    /// Store package by its name (the part of the family name before `_`).
    pub fn match_package(&self, package_name: &str) -> Option<&DbEntry> {
        self.entries
            .iter()
            .find(|e| any_match(&e.matcher.package_family, package_name))
    }

    /// Win32 app. With both name and publisher rules, both must match.
    pub fn match_win32(&self, name: &str, publisher: Option<&str>) -> Option<&DbEntry> {
        let publisher = publisher.unwrap_or("");
        self.entries.iter().find(|e| {
            let m = &e.matcher;
            let name_ok = any_match(&m.win32_name, name);
            let pub_ok = any_match(&m.win32_publisher, publisher);
            match (m.win32_name.is_empty(), m.win32_publisher.is_empty()) {
                (false, false) => name_ok && pub_ok,
                (false, true) => name_ok,
                (true, false) => pub_ok,
                (true, true) => false,
            }
        })
    }

    pub fn match_service(&self, service_name: &str) -> Option<&DbEntry> {
        self.entries.iter().find(|e| any_match(&e.matcher.service, service_name))
    }

    pub fn match_startup(&self, entry_name: &str) -> Option<&DbEntry> {
        self.entries.iter().find(|e| any_match(&e.matcher.startup, entry_name))
    }

    pub fn match_task(&self, task_path: &str) -> Option<&DbEntry> {
        self.entries.iter().find(|e| any_match(&e.matcher.task, task_path))
    }

    pub fn find(&self, id: &str) -> Option<&DbEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

impl DbEntry {
    /// Applies the entry's conditions. A condition can only make an entry more cautious.
    pub fn rate(&self, facts: ConditionFacts) -> Rated {
        let mut rated = Rated {
            safety: self.safety,
            recommendation: self.recommendation,
            notes: Vec::new(),
        };
        for c in &self.conditions {
            let applies = match c.when.as_str() {
                "games-installed" => facts.games_installed,
                "org-managed" => facts.org_managed,
                _ => false,
            };
            if !applies {
                continue;
            }
            if let Some(s) = c.safety {
                if s.rank() > rated.safety.rank() {
                    rated.safety = s;
                }
            }
            if let Some(r) = c.recommendation {
                let order = |r: Recommendation| match r {
                    Recommendation::Remove => 0,
                    Recommendation::Optional => 1,
                    Recommendation::Keep => 2,
                };
                if order(r) > order(rated.recommendation) {
                    rated.recommendation = r;
                }
            }
            if let Some(n) = &c.note {
                rated.notes.push(n.clone());
            }
        }
        rated
    }
}
