//! `ChangePlanner`: turns the ticked items into an ordered list of actions.
//! Order: restore point, startup entries, scheduled tasks, services, uninstalls.
//! Only items rated Safe to remove or Usually safe can ever produce an action, whatever is
//! passed in. Dry run executes the same plan, log-only.

use serde::{Deserialize, Serialize};

use super::w11d::TweakSet;
use super::{BloatItem, ItemKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum ActionKind {
    RestorePoint,
    DisableStartup {
        name: String,
        /// The Startup page's source key, e.g. "registry-run-machine".
        source: String,
        registry_key: Option<String>,
    },
    DisableTask {
        path: String,
    },
    ConfigureService {
        name: String,
        display_name: String,
        /// "Manual" or "Disabled".
        start_type: String,
        stop: bool,
    },
    UninstallStore {
        package_full_name: String,
        package_name: String,
    },
    RemoveProvisioned {
        package_name: String,
    },
    UninstallWin32 {
        display_name: String,
        registry_subkey: Option<String>,
    },
    /// Imports a Win11Debloat registry file.
    ApplyTweak {
        feature_id: String,
        label: String,
        registry_file: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: String,
    pub item_id: String,
    pub item_name: String,
    pub kind: ActionKind,
    pub description: String,
    pub needs_admin: bool,
    /// ASAO can put it back by itself.
    pub undoable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOptions {
    /// Also remove Store apps for new users on this PC (provisioned packages).
    pub remove_provisioned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejected {
    pub item_id: String,
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub actions: Vec<Action>,
    pub rejected: Vec<Rejected>,
    pub needs_admin: bool,
    pub freed_bytes: u64,
    pub services_stopped: usize,
    /// Names of items whose removal ASAO cannot undo by itself.
    pub not_undoable: Vec<String>,
}

fn machine_startup(source: &str) -> bool {
    source.ends_with("-machine")
}

pub fn plan(items: &[BloatItem], selected_ids: &[String], opts: &PlanOptions) -> Plan {
    plan_with_tweaks(items, selected_ids, &TweakSet::default(), &[], opts)
}

/// Apps, services and startup entries, then Win11Debloat tweaks. Only offered tweaks that
/// are not in effect yet, and one option per pick-one group.
pub fn plan_with_tweaks(
    items: &[BloatItem],
    selected_ids: &[String],
    tweaks: &TweakSet,
    selected_tweaks: &[String],
    opts: &PlanOptions,
) -> Plan {
    let mut out = plan_items(items, selected_ids, opts);
    let mut groups_used = std::collections::HashSet::new();
    let mut tweak_actions = Vec::new();
    for id in selected_tweaks {
        let Some(t) = tweaks.tweaks.iter().find(|t| &t.id == id) else {
            out.rejected.push(Rejected {
                item_id: id.clone(),
                name: id.clone(),
                reason: "It is not a setting ASAO offers.".into(),
            });
            continue;
        };
        if t.applied == Some(true) {
            continue;
        }
        if let Some(g) = &t.group_id {
            if !groups_used.insert(g.clone()) {
                continue;
            }
        }
        if tweak_actions.iter().any(|a: &Action| matches!(&a.kind, ActionKind::ApplyTweak { feature_id, .. } if feature_id == id)) {
            continue;
        }
        tweak_actions.push(Action {
            id: format!("tweak:{}", t.id),
            item_id: format!("tweak:{}", t.id),
            item_name: t.label.clone(),
            kind: ActionKind::ApplyTweak {
                feature_id: t.id.clone(),
                label: t.label.clone(),
                registry_file: t.registry_file.clone(),
            },
            description: format!("Change setting: {}", t.label),
            needs_admin: t.needs_admin,
            undoable: true,
        });
    }
    if !tweak_actions.is_empty() {
        if out.actions.is_empty() {
            out.actions.push(Action {
                id: "restore-point".into(),
                item_id: String::new(),
                item_name: "System Restore".into(),
                kind: ActionKind::RestorePoint,
                description: "Create a System Restore point".into(),
                needs_admin: true,
                undoable: false,
            });
        }
        out.actions.extend(tweak_actions);
    }
    out.needs_admin = out.actions.iter().any(|a| a.needs_admin);
    out
}

fn plan_items(items: &[BloatItem], selected_ids: &[String], opts: &PlanOptions) -> Plan {
    let mut out = Plan::default();
    let (mut startup, mut tasks, mut services, mut uninstalls, mut provisioned) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut seen = std::collections::HashSet::new();

    for id in selected_ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(item) = items.iter().find(|i| &i.id == id) else {
            out.rejected.push(Rejected {
                item_id: id.clone(),
                name: id.clone(),
                reason: "It is not in the latest scan.".into(),
            });
            continue;
        };
        if !item.safety.selectable() {
            out.rejected.push(Rejected {
                item_id: item.id.clone(),
                name: item.name.clone(),
                reason: format!("ASAO does not change items rated {}.", item.safety.label()),
            });
            continue;
        }
        let mut n = 0usize;
        let mut action = |kind: ActionKind, description: String, needs_admin: bool, undoable: bool| {
            n += 1;
            Action {
                id: format!("{}#{}", item.id, n),
                item_id: item.id.clone(),
                item_name: item.name.clone(),
                kind,
                description,
                needs_admin,
                undoable,
            }
        };

        for s in item.startup.iter().filter(|s| s.enabled) {
            startup.push(action(
                ActionKind::DisableStartup {
                    name: s.name.clone(),
                    source: s.source.clone(),
                    registry_key: s.registry_key.clone(),
                },
                format!("Disable the startup entry \"{}\"", s.name),
                machine_startup(&s.source),
                true,
            ));
        }
        for t in item.tasks.iter().filter(|t| t.enabled) {
            tasks.push(action(
                ActionKind::DisableTask { path: t.path.clone() },
                format!("Disable the scheduled task {}", t.path),
                true,
                true,
            ));
        }
        for s in &item.services {
            if s.start_type.eq_ignore_ascii_case("Disabled") {
                continue;
            }
            let running = s.state.eq_ignore_ascii_case("Running");
            if s.start_type.eq_ignore_ascii_case(&s.planned_start_type) && !running {
                continue;
            }
            if running || s.start_type.eq_ignore_ascii_case("Auto") {
                out.services_stopped += 1;
            }
            services.push(action(
                ActionKind::ConfigureService {
                    name: s.name.clone(),
                    display_name: s.display_name.clone(),
                    start_type: s.planned_start_type.clone(),
                    stop: running,
                },
                format!(
                    "{} \"{}\" and set it to {}",
                    if running { "Stop the service" } else { "Set the service" },
                    s.display_name,
                    s.planned_start_type
                ),
                true,
                true,
            ));
        }
        match item.kind {
            ItemKind::StoreApp => {
                if let (Some(full), Some(name)) = (item.package_full_name.clone(), item.package_name.clone()) {
                    uninstalls.push(action(
                        ActionKind::UninstallStore { package_full_name: full, package_name: name.clone() },
                        format!("Uninstall {} for your account", item.name),
                        false,
                        false,
                    ));
                    if opts.remove_provisioned {
                        provisioned.push(action(
                            ActionKind::RemoveProvisioned { package_name: name },
                            format!("Stop {} from being installed for new users", item.name),
                            true,
                            false,
                        ));
                    }
                }
                out.freed_bytes += item.size_bytes.unwrap_or(0);
            }
            ItemKind::Win32App => {
                uninstalls.push(action(
                    ActionKind::UninstallWin32 {
                        display_name: item.name.clone(),
                        registry_subkey: item.registry_subkey.clone(),
                    },
                    format!("Run {}'s own uninstaller", item.name),
                    false,
                    false,
                ));
                out.freed_bytes += item.size_bytes.unwrap_or(0);
                out.not_undoable.push(item.name.clone());
            }
            ItemKind::Service | ItemKind::StartupEntry => {}
        }
    }

    let mut actions: Vec<Action> = Vec::new();
    actions.extend(startup);
    actions.extend(tasks);
    actions.extend(services);
    actions.extend(uninstalls);
    actions.extend(provisioned);
    if !actions.is_empty() {
        actions.insert(
            0,
            Action {
                id: "restore-point".into(),
                item_id: String::new(),
                item_name: "System Restore".into(),
                kind: ActionKind::RestorePoint,
                description: "Create a System Restore point".into(),
                needs_admin: true,
                undoable: false,
            },
        );
    }
    out.needs_admin = actions.iter().any(|a| a.needs_admin);
    out.actions = actions;
    out
}

/// Phase of an action, for checking order: 0 restore point ... 5 provisioned.
pub fn phase(kind: &ActionKind) -> u8 {
    match kind {
        ActionKind::RestorePoint => 0,
        ActionKind::DisableStartup { .. } => 1,
        ActionKind::DisableTask { .. } => 2,
        ActionKind::ConfigureService { .. } => 3,
        ActionKind::UninstallStore { .. } | ActionKind::UninstallWin32 { .. } => 4,
        ActionKind::RemoveProvisioned { .. } => 5,
        ActionKind::ApplyTweak { .. } => 6,
    }
}
