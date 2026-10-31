//! Win11Debloat's data, used the way Win11Debloat uses it: `Config/Apps.json` (apps, default
//! selection, rating, removal method, presets), `Config/Features.json` (tweaks pointing to
//! `.reg` files) and `Config/DefaultSettings.json` (the default selection of tweaks).
//!
//! The files ship inside ASAO (resources/win11debloat). The same layout in the app data folder
//! (`bloat\win11debloat`) replaces them without a rebuild.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::database::{DbEntry, DbMatch, ServiceAction};
use super::regfile::{all_match, decode, parse, queries_for, read_live, touches_machine, RegOp};
use super::{Category, Recommendation, Safety};

include!(concat!(env!("OUT_DIR"), "/w11d_regfiles.rs"));

const APPS_JSON: &str = include_str!("../../resources/win11debloat/Config/Apps.json");
const APPS_EN_JSON: &str = include_str!("../../resources/win11debloat/Config/Languages/en-US/Apps.json");
const FEATURES_JSON: &str = include_str!("../../resources/win11debloat/Config/Features.json");
const DEFAULTS_JSON: &str = include_str!("../../resources/win11debloat/Config/DefaultSettings.json");

/// Tweak categories ASAO does not offer: they can stop security updates or change how the
/// system itself behaves.
pub const EXCLUDED_CATEGORIES: &[&str] = &["Windows Update", "System", "Optional Windows Features"];

/// Win11Debloat's winget ids, and what they are on a PC.
const WINGET_TARGETS: &[(&str, &[&str], &[&str])] = &[
    // (id, Store package names, Win32 display names)
    ("XP9CXNGPPJ97XX", &["Microsoft.Copilot"], &[]),
    ("Microsoft.OneDrive", &[], &["Microsoft OneDrive"]),
    ("Microsoft.Edge", &["Microsoft.MicrosoftEdge.Stable"], &["Microsoft Edge"]),
    ("XPFFTQ037JWMHS", &["Microsoft.MicrosoftEdge.Stable"], &["Microsoft Edge"]),
];

#[derive(Debug, Clone)]
pub struct W11App {
    pub ids: Vec<String>,
    pub selected_by_default: bool,
    /// "safe", "optional" or "unsafe".
    pub rating: String,
    /// "Appx" or "WinGet".
    pub method: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    pub app_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Feature {
    pub id: String,
    pub label: String,
    pub tooltip: Option<String>,
    pub category: Option<String>,
    pub registry_key: Option<String>,
    pub requires_reboot: bool,
    pub min_version: Option<u32>,
    pub max_version: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct UiGroup {
    pub id: String,
    pub label: String,
    pub tooltip: Option<String>,
    pub category: Option<String>,
    /// (label, feature ids)
    pub options: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone)]
pub struct Win11Debloat {
    pub apps: Vec<W11App>,
    pub presets: Vec<Preset>,
    pub features: Vec<Feature>,
    pub groups: Vec<UiGroup>,
    /// Feature ids Win11Debloat's default mode selects.
    pub defaults: HashSet<String>,
    /// "Built in", or the override folder.
    pub source: String,
    override_dir: Option<PathBuf>,
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty())
}
fn n(v: &Value, k: &str) -> Option<u32> {
    v.get(k).and_then(|x| x.as_u64()).map(|x| x as u32)
}
fn json(text: &str) -> Result<Value, String> {
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

impl Win11Debloat {
    fn parse(apps: &str, apps_en: &str, features: &str, defaults: &str) -> Result<Self, String> {
        let apps_v = json(apps).map_err(|e| format!("Apps.json: {}", e))?;
        let en = json(apps_en).unwrap_or(Value::Null);
        let feats_v = json(features).map_err(|e| format!("Features.json: {}", e))?;
        let defs_v = json(defaults).map_err(|e| format!("DefaultSettings.json: {}", e))?;

        let mut list = Vec::new();
        for a in apps_v["Apps"].as_array().ok_or("Apps.json has no Apps list")? {
            let ids: Vec<String> = match &a["AppId"] {
                Value::String(x) => vec![x.clone()],
                Value::Array(xs) => xs.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect(),
                _ => continue,
            };
            if ids.is_empty() {
                continue;
            }
            let text = ids.iter().find_map(|i| en.get(i)).cloned().unwrap_or(Value::Null);
            list.push(W11App {
                name: s(&text, "FriendlyName").unwrap_or_else(|| ids[0].clone()),
                description: s(&text, "Description").unwrap_or_default(),
                selected_by_default: a["SelectedByDefault"].as_bool().unwrap_or(false),
                rating: s(a, "Recommendation").unwrap_or_else(|| "optional".into()).to_lowercase(),
                method: s(a, "RemovalMethod").unwrap_or_else(|| "Appx".into()),
                ids,
            });
        }
        if list.is_empty() {
            return Err("Apps.json lists no apps".into());
        }
        let presets = apps_v["Presets"]
            .as_array()
            .map(|ps| {
                ps.iter()
                    .filter_map(|p| {
                        Some(Preset {
                            name: s(p, "Name")?,
                            app_ids: p["AppIds"]
                                .as_array()?
                                .iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let features = feats_v["Features"]
            .as_array()
            .ok_or("Features.json has no Features list")?
            .iter()
            .filter_map(|f| {
                Some(Feature {
                    id: s(f, "FeatureId")?,
                    label: s(f, "Label").unwrap_or_default(),
                    tooltip: s(f, "ToolTip"),
                    category: s(f, "Category"),
                    registry_key: s(f, "RegistryKey"),
                    requires_reboot: f["RequiresReboot"].as_bool().unwrap_or(false),
                    min_version: n(f, "MinVersion"),
                    max_version: n(f, "MaxVersion"),
                })
            })
            .collect();
        let groups = feats_v["UiGroups"]
            .as_array()
            .map(|gs| {
                gs.iter()
                    .filter_map(|g| {
                        Some(UiGroup {
                            id: s(g, "GroupId")?,
                            label: s(g, "Label").unwrap_or_default(),
                            tooltip: s(g, "ToolTip"),
                            category: s(g, "Category"),
                            options: g["Values"]
                                .as_array()?
                                .iter()
                                .filter_map(|v| {
                                    Some((
                                        s(v, "Label")?,
                                        v["FeatureIds"]
                                            .as_array()?
                                            .iter()
                                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                            .collect(),
                                    ))
                                })
                                .collect(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let defaults = defs_v["Settings"]
            .as_array()
            .map(|ss| {
                ss.iter()
                    .filter(|x| x["Value"].as_bool() == Some(true))
                    .filter_map(|x| s(x, "Name"))
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self { apps: list, presets, features, groups, defaults, source: "Built in".into(), override_dir: None })
    }

    pub fn built_in() -> Self {
        Self::parse(APPS_JSON, APPS_EN_JSON, FEATURES_JSON, DEFAULTS_JSON).expect("the bundled Win11Debloat files are valid")
    }

    /// The override folder when it holds a valid copy, otherwise the bundled files.
    pub fn load(override_dir: &Path) -> (Self, Option<String>) {
        let cfg = override_dir.join("Config");
        if !cfg.join("Apps.json").exists() {
            return (Self::built_in(), None);
        }
        let read = |p: PathBuf| std::fs::read_to_string(&p).unwrap_or_default();
        match Self::parse(
            &read(cfg.join("Apps.json")),
            &read(cfg.join("Languages").join("en-US").join("Apps.json")),
            &read(cfg.join("Features.json")),
            &read(cfg.join("DefaultSettings.json")),
        ) {
            Ok(mut w) => {
                w.source = override_dir.to_string_lossy().to_string();
                w.override_dir = Some(override_dir.to_path_buf());
                (w, None)
            }
            Err(e) => (Self::built_in(), Some(format!("Ignored the Win11Debloat files in {}: {}", override_dir.display(), e))),
        }
    }

    /// A registry file by name: from the override folder when present, else the bundled copy.
    pub fn regfile(&self, name: &str) -> Option<Vec<u8>> {
        if let Some(dir) = &self.override_dir {
            if let Ok(b) = std::fs::read(dir.join("Regfiles").join(name)) {
                return Some(b);
            }
        }
        REGFILES
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, b)| b.to_vec())
    }

    fn preset_of(&self, id: &str) -> Option<&str> {
        self.presets
            .iter()
            .find(|p| p.app_ids.iter().any(|a| a.eq_ignore_ascii_case(id)))
            .map(|p| p.name.as_str())
    }

    /// Win11Debloat's apps as database entries, so every ASAO check applies to them.
    pub fn db_entries(&self) -> Vec<DbEntry> {
        self.apps
            .iter()
            .map(|a| {
                let first = &a.ids[0];
                let mut matcher = DbMatch::default();
                if a.method.eq_ignore_ascii_case("WinGet") {
                    for id in &a.ids {
                        if let Some((_, pkgs, names)) = WINGET_TARGETS.iter().find(|(w, _, _)| w.eq_ignore_ascii_case(id)) {
                            matcher.package_family.extend(pkgs.iter().map(|p| p.to_string()));
                            matcher.win32_name.extend(names.iter().map(|n| n.to_string()));
                        }
                    }
                } else {
                    // Win11Debloat removes every package whose name contains the id.
                    matcher.package_family = a.ids.iter().map(|i| format!("*{}*", i)).collect();
                }
                let (safety, recommendation, reason) = match a.rating.as_str() {
                    "safe" => (Safety::Safe, Recommendation::Remove, "Win11Debloat rates it safe to remove."),
                    "unsafe" => (
                        Safety::NotRecommended,
                        Recommendation::Keep,
                        "Win11Debloat rates it unsafe to remove: Windows or other apps rely on it.",
                    ),
                    _ => (
                        Safety::UsuallySafe,
                        Recommendation::Optional,
                        "Win11Debloat rates it optional: nothing in Windows needs it, but some people use it.",
                    ),
                };
                let preset = self.preset_of(first);
                let category = if preset.map(|p| p.to_lowercase().contains("oem")).unwrap_or(false) {
                    Category::OemUtility
                } else if first.starts_with("Microsoft") || first.starts_with("Clipchamp") || first.starts_with("MSTeams") || first.starts_with("XP") {
                    Category::MicrosoftInboxApp
                } else {
                    Category::PromotedApp
                };
                let desc = a.description.trim_end_matches('.');
                let discontinued = desc.to_lowercase().contains("discontinued") || desc.to_lowercase().contains("deprecated");
                DbEntry {
                    id: format!("w11d-{}", slug(first)),
                    matcher,
                    display_name: a.name.clone(),
                    category,
                    what_it_does: if a.description.is_empty() { format!("{}.", a.name) } else { a.description.clone() },
                    safety,
                    safety_reason: reason.into(),
                    what_breaks: if discontinued {
                        format!("Nothing you are likely to miss: {} is no longer developed.", a.name)
                    } else if desc.is_empty() {
                        format!("You lose {}.", a.name)
                    } else {
                        format!("You lose {} ({}).", a.name, desc)
                    },
                    how_to_restore: if a.method.eq_ignore_ascii_case("WinGet") {
                        format!("Reinstall {} from its publisher, or with winget.", a.name)
                    } else {
                        format!("Reinstall {} from the Microsoft Store.", a.name)
                    },
                    recommendation,
                    conditions: Vec::new(),
                    service_action: ServiceAction::Manual,
                    source: format!(
                        "Win11Debloat (github.com/Raphire/Win11Debloat, MIT): Config/Apps.json{}",
                        preset.map(|p| format!(", preset \"{}\"", p)).unwrap_or_default()
                    ),
                    selected_by_default: a.selected_by_default,
                }
            })
            .collect()
    }

    /// Why a tweak is not offered on this PC, or `None` when it is.
    pub fn unavailable(&self, f: &Feature, build: Option<u32>) -> Option<String> {
        let Some(cat) = f.category.as_deref() else {
            return Some("It is an option of Win11Debloat itself, not a Windows setting.".into());
        };
        if EXCLUDED_CATEGORIES.contains(&cat) {
            return Some(format!("{} tweaks are not offered: they can stop security updates or change core system behaviour.", cat));
        }
        let Some(key) = f.registry_key.as_deref() else {
            return Some("Win11Debloat does this with a script, not a registry file.".into());
        };
        if self.regfile(key).is_none() {
            return Some(format!("Its registry file {} is missing.", key));
        }
        if let Some(b) = build {
            if f.min_version.map(|m| b < m).unwrap_or(false) || f.max_version.map(|m| b > m).unwrap_or(false) {
                return Some("It does not apply to this version of Windows.".into());
            }
        }
        None
    }
}

/// One tweak as shown to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakItem {
    pub id: String,
    pub label: String,
    pub tooltip: Option<String>,
    pub category: String,
    /// Set when the tweak is an option of a pick-one group.
    pub group_id: Option<String>,
    /// Every value in its registry file already matches. `None` when it could not be read.
    pub applied: Option<bool>,
    /// Win11Debloat's default mode selects it (and it is not applied yet).
    pub preselected: bool,
    pub requires_reboot: bool,
    /// It writes to HKEY_LOCAL_MACHINE.
    pub needs_admin: bool,
    pub registry_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakOption {
    pub label: String,
    pub feature_id: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakGroup {
    pub id: String,
    pub label: String,
    pub tooltip: Option<String>,
    pub category: String,
    pub options: Vec<TweakOption>,
    /// The option in effect now, if one matches.
    pub current: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TweakSet {
    pub tweaks: Vec<TweakItem>,
    pub groups: Vec<TweakGroup>,
    /// (label, why it is not offered)
    pub not_offered: Vec<(String, String)>,
}

impl Win11Debloat {
    /// Every tweak offered on this PC, with whether it is already in effect.
    pub fn tweaks(&self, build: Option<u32>, check_registry: bool) -> TweakSet {
        let grouped: HashMap<&str, &str> = self
            .groups
            .iter()
            .flat_map(|g| g.options.iter().flat_map(move |(_, ids)| ids.iter().map(move |i| (i.as_str(), g.id.as_str()))))
            .collect();
        let mut set = TweakSet::default();
        let mut offered: Vec<(&Feature, Vec<RegOp>)> = Vec::new();
        for f in &self.features {
            match self.unavailable(f, build) {
                Some(why) => {
                    if f.category.is_some() {
                        set.not_offered.push((f.label.clone(), why));
                    }
                }
                None => {
                    let ops = self
                        .regfile(f.registry_key.as_deref().unwrap_or(""))
                        .map(|b| parse(&decode(&b)))
                        .unwrap_or_default();
                    offered.push((f, ops));
                }
            }
        }
        // One registry read for every value of every offered tweak.
        let mut applied: Vec<Option<bool>> = vec![None; offered.len()];
        if check_registry {
            let queries: Vec<(String, Option<String>)> = offered.iter().flat_map(|(_, ops)| queries_for(ops)).collect();
            if let Ok(live) = read_live(&queries) {
                let mut at = 0usize;
                for (i, (_, ops)) in offered.iter().enumerate() {
                    let slice = &live[at.min(live.len())..(at + ops.len()).min(live.len())];
                    applied[i] = Some(all_match(ops, slice));
                    at += ops.len();
                }
            }
        }

        for (i, (f, ops)) in offered.iter().enumerate() {
            let group = grouped.get(f.id.as_str()).map(|g| g.to_string());
            set.tweaks.push(TweakItem {
                id: f.id.clone(),
                label: f.label.clone(),
                tooltip: f.tooltip.clone(),
                category: f.category.clone().unwrap_or_default(),
                preselected: group.is_none() && self.defaults.contains(&f.id) && applied[i] != Some(true),
                group_id: group,
                applied: applied[i],
                requires_reboot: f.requires_reboot,
                needs_admin: touches_machine(ops),
                registry_file: f.registry_key.clone().unwrap_or_default(),
            });
        }

        let offered_ids: HashSet<&str> = set.tweaks.iter().map(|t| t.id.as_str()).collect();
        for g in &self.groups {
            let options: Vec<TweakOption> = g
                .options
                .iter()
                .filter(|(_, ids)| !ids.is_empty() && ids.iter().all(|i| offered_ids.contains(i.as_str())))
                .map(|(label, ids)| TweakOption {
                    label: label.clone(),
                    feature_id: ids[0].clone(),
                    is_default: label.contains("(Default)"),
                })
                .collect();
            if options.len() < 2 {
                continue;
            }
            let current = options
                .iter()
                .find(|o| set.tweaks.iter().any(|t| t.id == o.feature_id && t.applied == Some(true)))
                .map(|o| o.feature_id.clone());
            set.groups.push(TweakGroup {
                id: g.id.clone(),
                label: g.label.clone(),
                tooltip: g.tooltip.clone(),
                category: g.category.clone().unwrap_or_default(),
                options,
                current,
            });
        }
        // Options of groups that were dropped are not offered on their own either.
        let kept_groups: HashSet<&str> = set.groups.iter().map(|g| g.id.as_str()).collect();
        set.tweaks.retain(|t| t.group_id.as_deref().map(|g| kept_groups.contains(g)).unwrap_or(true));
        set
    }

    /// The registry operations of an offered tweak.
    pub fn ops_for(&self, feature_id: &str) -> Option<(String, Vec<RegOp>, Vec<u8>)> {
        let f = self.features.iter().find(|f| f.id == feature_id)?;
        let key = f.registry_key.clone()?;
        let bytes = self.regfile(&key)?;
        Some((key, parse(&decode(&bytes)), bytes))
    }
}
