//! Bloatware review: preinstalled apps, OEM extras, trialware and the background services,
//! startup entries and scheduled tasks that come with them.
//!
//! - `database`   — `BloatwareDatabase`: the curated `bloatware-db.json`, match rules and conditions
//! - `inventory`  — apps, services, startup entries, scheduled tasks and device facts
//! - `impact`     — `ImpactScorer`, `ResourceSampler` and `StartupImpactReader`
//! - `thresholds` — every point threshold of the impact score, in one place
//! - `safety`     — `SafetyEvaluator`: database level plus runtime downgrade checks
//! - `scan`       — links everything to its owning app and builds the reviewed list
//!
//! Core rule: ASAO only recommends what the database positively knows. Anything unknown,
//! uncertain or needed is shown for information and can never be selected.

pub mod database;
pub mod elevated;
pub mod executor;
pub mod history;
pub mod impact;
pub mod inventory;
pub mod planner;
pub mod regfile;
pub mod safety;
pub mod scan;
pub mod thresholds;
pub mod w11d;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub const EVENT_BLOAT_SCAN_PROGRESS: &str = "BLOAT_SCAN_PROGRESS";
pub const EVENT_BLOAT_APPLY_PROGRESS: &str = "BLOAT_APPLY_PROGRESS";

/// Command-line flag that forces dry run (for release builds).
pub const DRY_RUN_FLAG: &str = "--bloat-dry-run";

/// Dry run forced by the `bloatDryRun` setting, `--bloat-dry-run` or `ASAO_BLOAT_DRY_RUN=1`.
pub fn dry_run_forced(setting: bool) -> bool {
    setting
        || std::env::args().any(|a| a == DRY_RUN_FLAG)
        || std::env::var("ASAO_BLOAT_DRY_RUN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
}

/// Development builds: dry run is on by default and the toggle is shown, so the request
/// decides. Release builds: off and hidden, unless forced.
pub fn effective_dry_run(requested: bool, dev_mode: bool, forced: bool) -> bool {
    forced || (dev_mode && requested)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Safety {
    Safe,
    UsuallySafe,
    Caution,
    NotRecommended,
    Protected,
    NotRated,
}

impl Safety {
    /// Higher is more restrictive. "Not rated" sits with Caution: a runtime check can still
    /// make it Not recommended or Protected, but never removable.
    pub fn rank(self) -> u8 {
        match self {
            Safety::Safe => 0,
            Safety::UsuallySafe => 1,
            Safety::Caution | Safety::NotRated => 2,
            Safety::NotRecommended => 3,
            Safety::Protected => 4,
        }
    }

    /// Only items ASAO is sure about can be ticked. Caution and Not rated are information only.
    pub fn selectable(self) -> bool {
        matches!(self, Safety::Safe | Safety::UsuallySafe)
    }

    pub fn label(self) -> &'static str {
        match self {
            Safety::Safe => "Safe to remove",
            Safety::UsuallySafe => "Usually safe",
            Safety::Caution => "Caution",
            Safety::NotRecommended => "Not recommended",
            Safety::Protected => "Protected",
            Safety::NotRated => "Not rated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Recommendation {
    Remove,
    Optional,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    MicrosoftInboxApp,
    OemUtility,
    Trialware,
    PromotedGame,
    PromotedApp,
    UpdaterHelper,
    OptionalWindowsService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    StoreApp,
    Win32App,
    Service,
    StartupEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImpactBand {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub binary_path: Option<String>,
    /// "Auto", "Manual", "Disabled" (as Windows reports it).
    pub start_type: String,
    pub delayed_auto_start: bool,
    pub state: String,
    pub account: Option<String>,
    pub process_id: u32,
    pub depends_on: Vec<String>,
    pub dependents: Vec<String>,
    /// The service program lives in the Windows folder.
    pub is_windows: bool,
    /// What ASAO would set the start type to: "Manual" or "Disabled".
    pub planned_start_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEntryInfo {
    pub id: String,
    pub name: String,
    /// The Startup page's source key, e.g. "registry-run-user", "startup-folder-user".
    pub source: String,
    pub command: Option<String>,
    pub executable: Option<String>,
    pub enabled: bool,
    pub registry_key: Option<String>,
    /// Windows' own startup measurement, when it could be read.
    pub boot_cpu_ms: Option<u64>,
    pub boot_disk_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfo {
    /// Folder and name, e.g. `\HP\HP Analytics`.
    pub path: String,
    pub name: String,
    pub action: Option<String>,
    pub enabled: bool,
    /// Runs at logon or boot, or more often than once a day.
    pub frequent: bool,
    pub triggers: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactPart {
    pub points: u32,
    pub max: u32,
    /// The measured value in plain words, e.g. "1.4% average CPU over 30 s".
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Impact {
    pub total: u32,
    pub band: ImpactBand,
    pub startup: ImpactPart,
    pub cpu: ImpactPart,
    pub memory: ImpactPart,
    pub autostart: ImpactPart,
    pub disk: ImpactPart,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatItem {
    pub id: String,
    pub kind: ItemKind,
    pub name: String,
    pub publisher: Option<String>,
    pub category: Option<Category>,
    /// The database entry it matched, if any.
    pub db_id: Option<String>,
    pub what_it_does: Option<String>,
    pub what_breaks: Option<String>,
    pub how_to_restore: Option<String>,
    pub source: Option<String>,
    pub recommendation: Option<Recommendation>,
    pub safety: Safety,
    pub safety_reason: String,
    /// Runtime checks that changed or confirmed the safety level, in plain words.
    pub safety_checks: Vec<String>,
    pub selectable: bool,
    pub preselected: bool,
    pub package_family_name: Option<String>,
    pub package_full_name: Option<String>,
    /// Store package name (family name without the publisher id), for provisioned removal.
    pub package_name: Option<String>,
    /// Win32: the uninstall key's own name, used to find the uninstaller and confirm removal.
    pub registry_subkey: Option<String>,
    pub install_location: Option<String>,
    pub size_bytes: Option<u64>,
    pub services: Vec<ServiceInfo>,
    pub startup: Vec<StartupEntryInfo>,
    pub tasks: Vec<TaskInfo>,
    /// Services or apps outside this item that depend on it.
    pub dependents: Vec<String>,
    pub running: bool,
    pub last_used_ms: Option<u64>,
    pub impact: Impact,
    /// Everything ASAO would remove or change, in order.
    pub planned_changes: Vec<String>,
    pub undo: String,
    pub store_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AntivirusProduct {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceContext {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub org_managed: bool,
    pub managed_reason: Option<String>,
    pub games_installed: bool,
    pub antivirus: Vec<AntivirusProduct>,
    pub defender_present: bool,
    pub is_elevated: bool,
    /// True when Windows' own startup measurements (WDI) could be read.
    pub startup_measured: bool,
    /// Windows build number (e.g. 26200), for tweaks limited to some versions.
    pub windows_build: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatScanStats {
    pub apps: usize,
    pub services: usize,
    pub startup_entries: usize,
    pub tasks: usize,
    pub sample_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatScanResult {
    pub scan_id: u64,
    pub started_ms: u64,
    pub finished_ms: u64,
    pub items: Vec<BloatItem>,
    pub device: DeviceContext,
    pub stats: BloatScanStats,
    pub db_version: u32,
    pub db_updated: String,
    /// "Built in" or the path of the override file in use.
    pub db_source: String,
    /// Win11Debloat's tweaks offered on this PC, with their current state.
    pub tweaks: w11d::TweakSet,
    /// Win11Debloat's app presets ("Xbox gaming apps", "OEM software").
    pub presets: Vec<w11d::Preset>,
    /// "Built in", or the folder of the Win11Debloat files in use.
    pub w11d_source: String,
    pub log: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BloatScanProgress {
    pub phase: String,
    pub message: String,
    /// 0-100 during the CPU sampling window.
    pub percent: Option<u8>,
}
