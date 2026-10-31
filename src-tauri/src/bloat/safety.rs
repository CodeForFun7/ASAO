//! `SafetyEvaluator`: the database level, then runtime checks that can only make an item
//! more cautious (never less), then the pre-selection rule.

use super::database::{ConditionFacts, DbEntry};
use super::thresholds::RECENT_USE_DAYS;
use super::{BloatItem, Category, ItemKind, Recommendation, Safety};

/// Store packages that are always kept, whatever the database says.
const ALWAYS_KEEP_PACKAGES: &[&str] = &[
    "microsoft.windowsstore", "microsoft.desktopappinstaller", "microsoft.sechealthui",
    "microsoft.vclibs", "microsoft.ui.xaml", "microsoft.net.native", "microsoft.windowsappruntime",
    "microsoftcorporationii.winappruntime", "microsoft.services.store.engagement",
    "microsoft.gamingservices",
];

/// Win32 names that are runtimes, drivers or security. Lowercase prefixes or fragments.
const ALWAYS_KEEP_WIN32_PREFIXES: &[&str] = &[
    "microsoft visual c++", "microsoft .net", "microsoft windows desktop runtime",
    "microsoft asp.net core", "microsoft edge webview2", "windows driver package", "windows software development kit",
];
const ALWAYS_KEEP_WIN32_WORDS: &[&str] = &[" driver", "chipset", "firmware", "bios", "runtime", "redistributable"];

#[derive(Debug, Clone)]
pub struct RuntimeFacts<'a> {
    pub conditions: ConditionFacts,
    /// Registered antivirus products that are switched on.
    pub active_antivirus: &'a [String],
    pub defender_present: bool,
    pub managed_reason: Option<&'a str>,
    pub now_ms: u64,
}

pub struct Verdict {
    pub safety: Safety,
    pub reason: String,
    pub recommendation: Option<Recommendation>,
    pub checks: Vec<String>,
}

/// Only ever raises the level. "Not rated" stays "Not rated" under a Caution check:
/// both are information only.
fn tighten(v: &mut Verdict, to: Safety, why: String) {
    if to.rank() > v.safety.rank() {
        v.safety = to;
    }
    v.checks.push(why);
}

/// Is this app one of the always-kept platform pieces? Returns the reason.
pub fn always_kept(kind: ItemKind, package_name: Option<&str>, name: &str, is_framework: bool) -> Option<String> {
    if is_framework {
        return Some("It is a framework package other apps are built on.".into());
    }
    if let Some(p) = package_name {
        let p = p.to_lowercase();
        if ALWAYS_KEEP_PACKAGES.iter().any(|k| p.starts_with(k)) {
            return Some("Windows needs it to install, update or run apps, or to keep the PC secure.".into());
        }
    }
    if kind == ItemKind::Win32App {
        let n = name.to_lowercase();
        if ALWAYS_KEEP_WIN32_PREFIXES.iter().any(|k| n.starts_with(k))
            || ALWAYS_KEEP_WIN32_WORDS.iter().any(|w| n.contains(w))
        {
            return Some("It is a runtime, driver or firmware component that other software or hardware needs.".into());
        }
    }
    None
}

/// Database level and conditions; "Not rated" when the item is not in the database.
pub fn base_verdict(entry: Option<&DbEntry>, facts: &RuntimeFacts) -> Verdict {
    match entry {
        Some(e) => {
            let rated = e.rate(facts.conditions);
            let mut reason = e.safety_reason.clone();
            if rated.safety != e.safety {
                reason = format!("{} {}", reason, rated.notes.join(" ")).trim().to_string();
            }
            Verdict {
                safety: rated.safety,
                reason,
                recommendation: Some(rated.recommendation),
                checks: rated.notes,
            }
        }
        None => Verdict {
            safety: Safety::NotRated,
            reason: "It is not in ASAO's list of known bloatware, so ASAO makes no recommendation and will not change it.".into(),
            recommendation: None,
            checks: Vec::new(),
        },
    }
}

/// Runtime downgrades. `kept` is the always-keep reason, `protected` Windows' own lock.
pub fn apply_runtime_checks(
    v: &mut Verdict,
    item: &BloatItem,
    facts: &RuntimeFacts,
    kept: Option<String>,
    protected: Option<String>,
) {
    if let Some(why) = protected {
        tighten(v, Safety::Protected, why);
    }
    if let Some(why) = kept {
        tighten(v, Safety::NotRecommended, why);
    }

    // Something outside this item depends on it.
    if !item.dependents.is_empty() {
        tighten(
            v,
            Safety::Caution,
            format!("Other services depend on it: {}.", item.dependents.join(", ")),
        );
    }

    // The active antivirus.
    let name = item.name.to_lowercase();
    let publisher = item.publisher.as_deref().unwrap_or("").to_lowercase();
    for av in facts.active_antivirus {
        let first = av.split_whitespace().next().unwrap_or("").to_lowercase();
        if first.len() < 3 {
            continue;
        }
        let is_defender = av.to_lowercase().contains("defender");
        if (name.contains(&first) || publisher.contains(&first)) && !is_defender {
            if facts.defender_present {
                tighten(
                    v,
                    Safety::Caution,
                    format!("{} is your active antivirus. Windows Defender would take over if it were removed.", av),
                );
            } else {
                tighten(v, Safety::NotRecommended, format!("{} is your active antivirus.", av));
            }
        }
    }

    // In use now, or recently.
    if item.running {
        tighten(v, Safety::Caution, "It is running right now.".into());
    } else if let Some(last) = item.last_used_ms {
        let days = facts.now_ms.saturating_sub(last) / 86_400_000;
        if days < RECENT_USE_DAYS {
            tighten(
                v,
                Safety::Caution,
                if days == 0 {
                    "You used it today.".into()
                } else {
                    format!("You used it {} {} ago.", days, if days == 1 { "day" } else { "days" })
                },
            );
        }
    }

    // Managed PCs: the organisation may rely on OEM support and management tools.
    if item.category == Some(Category::OemUtility) {
        if let Some(why) = facts.managed_reason {
            tighten(v, Safety::Caution, format!("This PC is managed by an organisation ({}).", why));
        }
    }

    if !v.safety.selectable() && v.recommendation == Some(Recommendation::Remove) {
        v.recommendation = Some(Recommendation::Keep);
    }
}

/// Pre-ticked exactly as Win11Debloat's default selection does, but only when the item can be
/// ticked at all (ASAO's checks still apply). Services are never pre-ticked.
pub fn preselected(item: &BloatItem, selected_by_default: bool) -> bool {
    selected_by_default
        && item.safety.selectable()
        && item.kind != ItemKind::Service
        && item.category != Some(Category::OptionalWindowsService)
}
