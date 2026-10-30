//! Unused apps: installed apps you have not used in a while, with a safe way to remove each.
//!
//! - `usage`   — when was each program last used (UserAssist, Prefetch, ASAO's own monitor)
//! - `scan`    — builds the list: registered apps, Store apps, and apps that were never registered
//! - `actions` — the removal paths: the app's uninstaller, Windows Installer, Windows Settings,
//!               Store package removal, the Steam/Epic launcher, or removing a program folder
//!
//! Nothing is ever uninstalled silently: every action starts from the user's click and either
//! runs the app's own uninstaller (with its normal window) or asks Windows to do it.

pub mod actions;
pub mod scan;
pub mod usage;

use serde::{Deserialize, Serialize};

pub const EVENT_APPS_UNINSTALL_FINISHED: &str = "APPS_UNINSTALL_FINISHED";
pub const EVENT_APPS_SIZE_UPDATE: &str = "APPS_SIZE_UPDATE";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppKind {
    /// Has an entry in Windows' installed-apps list.
    Registered,
    /// A Microsoft Store (MSIX) app.
    Store,
    /// Never registered: a portable app, a copied game, an unzipped tool.
    Unregistered,
    Steam,
    Epic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemovalMethod {
    /// Run the app's own uninstaller.
    Uninstaller,
    /// Ask Windows Installer to remove the product (`msiexec /x {code}`).
    MsiProduct,
    /// Open Windows Settings, Apps (the uninstaller is missing).
    WindowsSettings,
    /// Remove the Store package.
    StorePackage,
    /// Move the program folder to the Recycle Bin (apps that were never registered).
    RemoveFolder,
    /// Hand the game to Steam's own uninstall.
    SteamUninstall,
    /// Open the Epic Games Launcher, which manages the game.
    OpenEpic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UsageSource {
    /// Windows' record of programs launched from Explorer or Start (UserAssist).
    LaunchRecord,
    /// Windows Prefetch (readable only as administrator).
    Prefetch,
    /// ASAO saw the program running.
    AsaoMonitor,
}

/// Facts only the backend uses to carry out an action. Never sent to the page.
#[derive(Debug, Clone, Default)]
pub struct AppInternal {
    pub uninstall_string: Option<String>,
    pub msi_product_code: Option<String>,
    pub package_full_name: Option<String>,
    pub steam_app_id: Option<String>,
    /// Registry app id ("hklm:{guid}"), used to watch for the uninstall to finish.
    pub registry_id: Option<String>,
    /// History key (normalised name and publisher), used for the ignore list.
    pub history_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnusedApp {
    pub id: String,
    /// Stable key for "don't suggest again".
    pub key: String,
    pub name: String,
    pub publisher: Option<String>,
    pub kind: AppKind,
    pub method: RemovalMethod,
    /// A second way to remove it when the main one cannot work (broken uninstaller).
    pub fallback: Option<RemovalMethod>,
    /// Plain-language note shown with the action ("The app's uninstaller is missing").
    pub note: Option<String>,
    pub last_used_ms: Option<u64>,
    pub last_used_source: Option<UsageSource>,
    pub installed_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub install_dir: Option<String>,
    pub needs_admin: bool,
    #[serde(skip)]
    pub internal: AppInternal,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnusedAppsResult {
    pub scan_id: u64,
    pub apps: Vec<UnusedApp>,
    /// Prefetch could be read (ASAO runs as administrator).
    pub prefetch_available: bool,
    /// When ASAO's own usage tracking started.
    pub tracking_since_ms: Option<u64>,
    /// Apps looked at, including ones not listed.
    pub considered: usize,
    /// Apps left out because they are running right now.
    pub running_skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSizeUpdate {
    pub scan_id: u64,
    pub id: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallFinished {
    pub id: String,
    pub name: String,
    /// The app is gone from Windows' list.
    pub removed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionStatus {
    /// The uninstaller or Windows window was opened; ASAO watches for the app to disappear.
    Launched,
    /// Done right away (Store package removed, folder moved to the Recycle Bin).
    Done,
    /// The user said no to the administrator prompt.
    Declined,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionOutcome {
    pub id: String,
    pub status: ActionStatus,
    pub message: String,
    /// ASAO is watching for the app to disappear from Windows' list.
    pub watching: bool,
}
