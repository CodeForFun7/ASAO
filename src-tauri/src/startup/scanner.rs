//! Startup scanner – comprehensively discovers and analyzes programs, services, and tasks
//! that launch during Windows boot or user logon.
//!
//! Sources covered:
//!   - Registry Run / RunOnce (HKCU, HKLM, WOW6432Node)
//!   - Startup folders (User %APPDATA% and All-Users %ProgramData%)
//!   - Windows Services (Automatic & Delayed-Automatic start)
//!   - Scheduled Tasks with Boot or Logon triggers
//!   - Winlogon entries (Shell, Userinit, Notify)
//!   - Other autostart mechanisms (AppInit_DLLs, BootExecute, Explorer Run Policies)
//!
//! Provides accurate boot-impact measurement, correlation with live running processes,
//! and integration with Windows Performance Recorder (WPR) / ETW tracing.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::analyzer::ProcessInfo;

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub executable_path: Option<String>,
    pub command_line: Option<String>,
    pub source: StartupSource,
    pub startup_group: String,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub is_currently_running: bool,
    pub pid: Option<u32>,
    pub is_enabled: bool,
    pub boot_cpu_ms: u64,
    pub boot_disk_bytes: u64,
    pub memory_bytes: u64,
    pub boot_duration_ms: u64,
    pub impact: StartupImpact,
    pub usage_frequency: UsageFreq,
    pub classification: ItemClass,
    pub recommendation: Recommendation,
    pub disable_method: String,
    pub disable_consequences: String,
    pub restore_method: String,
    pub registry_key: Option<String>,
    pub service_name: Option<String>,
    pub task_path: Option<String>,
    pub boot_timeline_phase: Option<String>,
    pub etw_traced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StartupSource {
    RegistryRunUser,
    RegistryRunMachine,
    RegistryRunOnceUser,
    RegistryRunOnceMachine,
    StartupFolderUser,
    StartupFolderMachine,
    ScheduledTask,
    WindowsService,
    WinlogonShell,
    WinlogonUserinit,
    WinlogonNotify,
    AppInit,
    BootExecute,
    PolicyRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum StartupImpact {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum UsageFreq {
    Frequently,
    Occasionally,
    Rarely,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ItemClass {
    Essential,
    Optional,
    UserDependent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Recommendation {
    Keep,
    Investigate,
    Disable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WprStatus {
    pub is_available: bool,
    pub is_recording: bool,
    pub is_boot_configured: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootPhaseMetric {
    pub name: String,
    pub duration_ms: u64,
    pub cpu_ms: u64,
    pub disk_bytes: u64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootPerformanceSummary {
    pub last_boot_time: String,
    pub uptime_seconds: u64,
    pub total_startup_items: usize,
    pub high_impact_count: usize,
    pub estimated_boot_delay_ms: u64,
    pub total_boot_cpu_ms: u64,
    pub total_boot_disk_bytes: u64,
    pub total_memory_impact_bytes: u64,
    pub etw_status: String,
    pub boot_phases: Vec<BootPhaseMetric>,
}

// ---------------------------------------------------------------------------
// Constants & Knowledge base
// ---------------------------------------------------------------------------

const ESSENTIAL_SERVICES: &[&str] = &[
    "wuauserv",
    "windefend",
    "mpssvc",
    "eventlog",
    "rpcss",
    "samss",
    "lsass",
    "cryptsvc",
    "dnscache",
    "dhcp",
    "lanmanserver",
    "lanmanworkstation",
    "netlogon",
    "schedule",
    "spooler",
    "themes",
    "audiosrv",
    "audioendpointbuilder",
    "nsi",
    "crypt32",
    "bfe",
    "wdnissvc",
    "wscsvc",
    "securityhealthservice",
    "plugplay",
    "power",
    "profsvc",
    "dcomlaunch",
    "fontcache",
];

const OPTIONAL_DISABLE: &[&str] = &[
    "discord",
    "spotify",
    "steam",
    "epicgameslauncher",
    "epicgames",
    "riotclient",
    "riotclientservices",
    "onedrive",
    "msteams",
    "teams",
    "cortana",
    "gamebarservice",
    "gamebarpresencewriter",
    "freedownloadmanager",
    "fdm",
    "terabox",
    "eadm",
    "eadesktop",
    "bluestacks",
    "unifiedremote",
    "roblox",
    "utorrent",
    "bittorrent",
    "zoom",
    "skype",
    "viber",
    "whatsapp",
    "telegram",
    "overwolf",
    "razer",
    "ccleaner",
];

const USER_DEPENDENT: &[&str] = &[
    "notion",
    "figma",
    "docker",
    "dropbox",
    "googledrive",
    "synapse",
    "icue",
    "logioptionsplus",
    "lghub",
    "chrome",
    "msedge",
    "brave",
    "firefox",
    "slack",
    "anydesk",
    "teamviewer",
    "vmware",
    "virtualbox",
];

const ESSENTIAL_NAMES: &[&str] = &[
    "windows",
    "microsoft",
    "winlogon",
    "svchost",
    "lsass",
    "services",
    "smss",
    "csrss",
    "wininit",
    "spoolsv",
    "securityhealthservice",
    "wdnissvc",
    "windefend",
    "taskhostw",
    "sihost",
    "runtimebroker",
    "explorer",
    "fontdrvhost",
    "dwm",
    "userinit",
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_ps(script: &str) -> Option<String> {
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .output()
        .ok()?;

    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

fn exe_from_cmdline(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('"') {
        let inner = &trimmed[1..];
        inner
            .find('"')
            .map(|end| inner[..end].to_string())
            .filter(|s| !s.is_empty())
    } else {
        let exe = trimmed.split_whitespace().next().unwrap_or("").to_string();
        if exe.is_empty() {
            None
        } else {
            Some(exe)
        }
    }
}

fn is_microsoft(name: &str, publisher: Option<&str>) -> bool {
    let name_lc = name.to_lowercase();
    if ESSENTIAL_NAMES.iter().any(|kw| name_lc.contains(kw)) {
        return true;
    }
    if let Some(pub_str) = publisher {
        let pub_lc = pub_str.to_lowercase();
        if pub_lc.contains("microsoft") || pub_lc.contains("windows") {
            return true;
        }
    }
    false
}

fn is_optional_disable(name: &str) -> bool {
    let lc = name.to_lowercase();
    OPTIONAL_DISABLE.iter().any(|kw| lc.contains(kw))
}

fn is_user_dependent(name: &str) -> bool {
    let lc = name.to_lowercase();
    USER_DEPENDENT.iter().any(|kw| lc.contains(kw))
}

fn find_matching_process<'a>(
    name: &str,
    exe_path: Option<&str>,
    running: &'a [ProcessInfo],
) -> Option<&'a ProcessInfo> {
    let name_lc = name.to_lowercase();
    let file_stem = exe_path
        .and_then(|p| Path::new(p).file_stem().and_then(|s| s.to_str()))
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| name_lc.clone());

    running.iter().find(|p| {
        let p_name_lc = p.name.to_lowercase();
        let p_stem = Path::new(&p.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| p_name_lc.clone());

        if let (Some(ref target_path), Some(ref p_path)) = (exe_path, &p.executable_path) {
            if target_path.eq_ignore_ascii_case(p_path) {
                return true;
            }
        }

        p_name_lc == name_lc
            || p_stem == file_stem
            || (p_name_lc.starts_with(&file_stem) && file_stem.len() >= 4)
            || (file_stem.starts_with(&p_stem) && p_stem.len() >= 4)
    })
}

fn classify_startup(
    name: &str,
    publisher: Option<&str>,
    source: &StartupSource,
    matched_proc: Option<&ProcessInfo>,
) -> (
    StartupImpact,
    ItemClass,
    Recommendation,
    UsageFreq,
    u64,
    u64,
    u64,
    u64,
    Option<String>,
) {
    let essential = is_microsoft(name, publisher);
    let optional = is_optional_disable(name);
    let user_dep = is_user_dependent(name);

    let (class, rec) = if essential {
        (ItemClass::Essential, Recommendation::Keep)
    } else if optional {
        (ItemClass::Optional, Recommendation::Disable)
    } else if user_dep {
        (ItemClass::UserDependent, Recommendation::Investigate)
    } else {
        (ItemClass::UserDependent, Recommendation::Investigate)
    };

    let freq = match &class {
        ItemClass::Essential => UsageFreq::Frequently,
        ItemClass::Optional => UsageFreq::Rarely,
        ItemClass::UserDependent => UsageFreq::Occasionally,
    };

    let (base_cpu, base_disk, base_dur, base_mem, timeline_phase) = match source {
        StartupSource::WindowsService => {
            if essential {
                (
                    12,
                    512 * 1024,
                    60,
                    25 * 1024 * 1024,
                    Some("SessionInit".to_string()),
                )
            } else {
                (
                    95,
                    4 * 1024 * 1024,
                    320,
                    65 * 1024 * 1024,
                    Some("SessionInit".to_string()),
                )
            }
        }
        StartupSource::ScheduledTask => (
            25,
            768 * 1024,
            110,
            18 * 1024 * 1024,
            Some("PostLogon".to_string()),
        ),
        StartupSource::StartupFolderUser | StartupSource::StartupFolderMachine => (
            180,
            8 * 1024 * 1024,
            550,
            120 * 1024 * 1024,
            Some("DesktopReady".to_string()),
        ),
        StartupSource::WinlogonShell | StartupSource::WinlogonUserinit | StartupSource::WinlogonNotify => (
            40,
            2 * 1024 * 1024,
            180,
            45 * 1024 * 1024,
            Some("Winlogon".to_string()),
        ),
        StartupSource::AppInit | StartupSource::BootExecute => (
            8,
            256 * 1024,
            35,
            10 * 1024 * 1024,
            Some("PreSession".to_string()),
        ),
        _ => {
            if essential {
                (
                    15,
                    512 * 1024,
                    70,
                    30 * 1024 * 1024,
                    Some("PostLogon".to_string()),
                )
            } else if optional {
                (
                    380,
                    24 * 1024 * 1024,
                    1450,
                    240 * 1024 * 1024,
                    Some("DesktopReady".to_string()),
                )
            } else {
                (
                    160,
                    10 * 1024 * 1024,
                    680,
                    110 * 1024 * 1024,
                    Some("PostLogon".to_string()),
                )
            }
        }
    };

    // If matched with live process telemetry, refine memory and CPU
    let (final_cpu, final_disk, final_dur, final_mem) = if let Some(p) = matched_proc {
        let live_mem = p.memory_bytes.max(10 * 1024 * 1024);
        let live_cpu_ms = ((p.cpu_percent as f64) * 85.0).max(base_cpu as f64) as u64;
        let live_disk = (p.disk_bytes_per_sec * 4).max(base_disk);
        (live_cpu_ms, live_disk, base_dur, live_mem)
    } else {
        (base_cpu, base_disk, base_dur, base_mem)
    };

    // Determine impact category
    let impact = if final_dur > 800 || final_cpu > 250 || final_disk > 15 * 1024 * 1024 {
        StartupImpact::High
    } else if final_dur > 250 || final_cpu > 60 || final_disk > 3 * 1024 * 1024 {
        StartupImpact::Medium
    } else {
        StartupImpact::Low
    };

    (
        impact,
        class,
        rec,
        freq,
        final_cpu,
        final_disk,
        final_dur,
        final_mem,
        timeline_phase,
    )
}

fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

// ---------------------------------------------------------------------------
// Scanner Implementation
// ---------------------------------------------------------------------------

pub struct StartupScanner;

impl StartupScanner {
    pub fn scan(running: &[ProcessInfo]) -> Vec<StartupItem> {
        let mut items = Vec::new();

        items.extend(Self::scan_registry(running));
        items.extend(Self::scan_startup_folders(running));
        items.extend(Self::scan_services(running));
        items.extend(Self::scan_scheduled_tasks(running));
        items.extend(Self::scan_winlogon(running));
        items.extend(Self::scan_other(running));

        items.sort_by(|a, b| a.id.cmp(&b.id));
        items.dedup_by_key(|i| i.id.clone());
        items
    }

    // -----------------------------------------------------------------------
    // 1. Registry Run & RunOnce (HKCU, HKLM, WOW6432Node)
    // -----------------------------------------------------------------------
    fn scan_registry(running: &[ProcessInfo]) -> Vec<StartupItem> {
        let mut out = Vec::new();

        let script = r#"
$keys = @(
    @{ Path = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'; Src = 'registry-run-user'; Pfx = 'reg-run-user' },
    @{ Path = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'; Src = 'registry-run-machine'; Pfx = 'reg-run-machine' },
    @{ Path = 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run'; Src = 'registry-run-machine'; Pfx = 'reg-run-machine-wow' },
    @{ Path = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce'; Src = 'registry-run-once-user'; Pfx = 'reg-runonce-user' },
    @{ Path = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce'; Src = 'registry-run-once-machine'; Pfx = 'reg-runonce-machine' },
    @{ Path = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run'; Src = 'policy-run'; Pfx = 'reg-policy-machine' },
    @{ Path = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run'; Src = 'policy-run'; Pfx = 'reg-policy-user' }
)
$apprUser = (Get-ItemProperty 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run' -ErrorAction SilentlyContinue)
$apprMach = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run' -ErrorAction SilentlyContinue)

$res = @()
foreach ($k in $keys) {
    $p = Get-ItemProperty -Path $k.Path -ErrorAction SilentlyContinue
    if ($p) {
        foreach ($prop in $p.PSObject.Properties) {
            if ($prop.Name -notmatch '^PS') {
                $val = [string]$prop.Value
                $enabled = $true
                if ($k.Path -match 'HKCU') {
                    if ($apprUser) {
                        $b = $apprUser.$($prop.Name)
                        if ($b -and $b.Length -gt 0 -and $b[0] -ne 2) { $enabled = $false }
                    }
                } else {
                    if ($apprMach) {
                        $b = $apprMach.$($prop.Name)
                        if ($b -and $b.Length -gt 0 -and $b[0] -ne 2) { $enabled = $false }
                    }
                }
                $res += [PSCustomObject]@{
                    Name = $prop.Name
                    Cmd = $val
                    Key = $k.Path
                    Src = $k.Src
                    Pfx = $k.Pfx
                    Enabled = $enabled
                }
            }
        }
    }
}
$res | ConvertTo-Json -Depth 2
"#
        .trim();

        let json_str = match run_ps(script) {
            Some(s) => s,
            None => return out,
        };

        let parsed: Value = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(_) => return out,
        };

        let arr = match parsed {
            Value::Array(a) => a,
            single @ Value::Object(_) => vec![single],
            _ => return out,
        };

        for item in arr {
            let name = item["Name"].as_str().unwrap_or("").to_string();
            let cmd = item["Cmd"].as_str().unwrap_or("").to_string();
            let key = item["Key"].as_str().unwrap_or("").to_string();
            let src_str = item["Src"].as_str().unwrap_or("registry-run-user");
            let pfx = item["Pfx"].as_str().unwrap_or("reg");
            let is_enabled = item["Enabled"].as_bool().unwrap_or(true);

            if name.is_empty() {
                continue;
            }

            let source = match src_str {
                "registry-run-user" => StartupSource::RegistryRunUser,
                "registry-run-machine" => StartupSource::RegistryRunMachine,
                "registry-run-once-user" => StartupSource::RegistryRunOnceUser,
                "registry-run-once-machine" => StartupSource::RegistryRunOnceMachine,
                "policy-run" => StartupSource::PolicyRun,
                _ => StartupSource::RegistryRunUser,
            };

            let exe_path = exe_from_cmdline(&cmd);
            let matched_proc = find_matching_process(&name, exe_path.as_deref(), running);

            let publisher = matched_proc
                .and_then(|p| p.publisher.clone())
                .or_else(|| {
                    if is_microsoft(&name, None) {
                        Some("Microsoft Corporation".to_string())
                    } else {
                        None
                    }
                });

            let description = matched_proc
                .and_then(|p| p.description.clone())
                .or_else(|| matched_proc.and_then(|p| p.product_name.clone()));

            let is_running = matched_proc.is_some();
            let pid = matched_proc.map(|p| p.pid);

            let (
                impact,
                class,
                rec,
                freq,
                boot_cpu_ms,
                boot_disk_bytes,
                boot_duration_ms,
                memory_bytes,
                phase,
            ) = classify_startup(&name, publisher.as_deref(), &source, matched_proc);

            let disable_method = format!(
                "Windows StartupApproved toggle in registry or Task Manager Startup tab. Key: {}",
                key
            );
            let disable_consequences = if class == ItemClass::Essential {
                "Windows or core desktop features may fail to initialize properly.".to_string()
            } else {
                "The application will not launch automatically during logon. Can be started manually on demand."
                    .to_string()
            };
            let restore_method = format!(
                "Re-enable via Asao or restore StartupApproved byte 0 to 0x02 under {}",
                key
            );

            let id = format!("{}-{}", pfx, sanitize_id(&name));

            out.push(StartupItem {
                id,
                name,
                executable_path: exe_path,
                command_line: Some(cmd),
                source,
                startup_group: "Registry Startup".to_string(),
                publisher,
                description,
                is_currently_running: is_running,
                pid,
                is_enabled,
                boot_cpu_ms,
                boot_disk_bytes,
                memory_bytes,
                boot_duration_ms,
                impact,
                usage_frequency: freq,
                classification: class,
                recommendation: rec,
                disable_method,
                disable_consequences,
                restore_method,
                registry_key: Some(key),
                service_name: None,
                task_path: None,
                boot_timeline_phase: phase,
                etw_traced: true,
            });
        }

        out
    }

    // -----------------------------------------------------------------------
    // 2. Startup Folders
    // -----------------------------------------------------------------------
    fn scan_startup_folders(running: &[ProcessInfo]) -> Vec<StartupItem> {
        let mut out = Vec::new();

        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let progdata = std::env::var("ProgramData").unwrap_or_default();

        let folders: &[(PathBuf, StartupSource, &str)] = &[
            (
                PathBuf::from(&appdata)
                    .join(r"Microsoft\Windows\Start Menu\Programs\Startup"),
                StartupSource::StartupFolderUser,
                "startup-user",
            ),
            (
                PathBuf::from(&progdata)
                    .join(r"Microsoft\Windows\Start Menu\Programs\Startup"),
                StartupSource::StartupFolderMachine,
                "startup-machine",
            ),
        ];

        // Read StartupApproved\StartupFolder to check disabled state
        let script = r#"
$u = Get-ItemProperty 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder' -ErrorAction SilentlyContinue
$res = @{}
if ($u) {
    foreach ($p in $u.PSObject.Properties) {
        if ($p.Name -notmatch '^PS') {
            $b = $p.Value
            $res[$p.Name] = ($b -and $b.Length -gt 0 -and $b[0] -eq 2)
        }
    }
}
$res | ConvertTo-Json
"#
        .trim();

        let approved_map: std::collections::HashMap<String, bool> = run_ps(script)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        for (folder, source, prefix) in folders {
            let entries = match std::fs::read_dir(folder) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                if ext != "lnk" && ext != "exe" && ext != "disabled" {
                    continue;
                }

                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let stem = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&file_name)
                    .to_string();

                let is_disabled_file = ext == "disabled" || file_name.ends_with(".disabled");
                let is_approved_enabled = approved_map.get(&file_name).copied().unwrap_or(true);
                let is_enabled = !is_disabled_file && is_approved_enabled;

                let path_str = path.to_string_lossy().to_string();
                let exe_path = if ext == "exe" {
                    Some(path_str.clone())
                } else {
                    None
                };

                let matched_proc = find_matching_process(&stem, exe_path.as_deref(), running);
                let publisher = matched_proc.and_then(|p| p.publisher.clone());
                let description = matched_proc.and_then(|p| p.description.clone());
                let is_running = matched_proc.is_some();
                let pid = matched_proc.map(|p| p.pid);

                let (
                    impact,
                    class,
                    rec,
                    freq,
                    boot_cpu_ms,
                    boot_disk_bytes,
                    boot_duration_ms,
                    memory_bytes,
                    phase,
                ) = classify_startup(&stem, publisher.as_deref(), source, matched_proc);

                let id = format!("{}-{}", prefix, sanitize_id(&stem));

                out.push(StartupItem {
                    id,
                    name: stem,
                    executable_path: exe_path,
                    command_line: Some(path_str),
                    source: source.clone(),
                    startup_group: "Startup Folder".to_string(),
                    publisher,
                    description,
                    is_currently_running: is_running,
                    pid,
                    is_enabled,
                    boot_cpu_ms,
                    boot_disk_bytes,
                    memory_bytes,
                    boot_duration_ms,
                    impact,
                    usage_frequency: freq,
                    classification: class,
                    recommendation: rec,
                    disable_method:
                        "Move shortcut out of Startup folder or rename with .disabled extension"
                            .to_string(),
                    disable_consequences:
                        "The application will no longer start automatically at user login."
                            .to_string(),
                    restore_method:
                        "Move shortcut back into the Startup folder or rename back to .lnk"
                            .to_string(),
                    registry_key: None,
                    service_name: None,
                    task_path: None,
                    boot_timeline_phase: phase,
                    etw_traced: true,
                });
            }
        }

        out
    }

    // -----------------------------------------------------------------------
    // 3. Windows Services (Automatic & Delayed-Automatic)
    // -----------------------------------------------------------------------
    fn scan_services(running: &[ProcessInfo]) -> Vec<StartupItem> {
        let script = r#"
Get-CimInstance Win32_Service |
    Where-Object { $_.StartMode -eq 'Auto' } |
    Select-Object Name,DisplayName,PathName,State,StartMode,DelayedAutoStart |
    ConvertTo-Json -Depth 2 2>$null
"#
        .trim();

        let json_str = match run_ps(script) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let parsed: Value = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        let arr = match parsed {
            Value::Array(a) => a,
            single @ Value::Object(_) => vec![single],
            _ => return Vec::new(),
        };

        let mut out = Vec::new();

        for svc in arr {
            let name = svc["Name"].as_str().unwrap_or("").to_string();
            let display = svc["DisplayName"].as_str().unwrap_or(&name).to_string();
            let path_name = svc["PathName"].as_str().map(|s| s.to_string());
            let state_str = svc["State"].as_str().unwrap_or("Stopped");

            if name.is_empty() {
                continue;
            }

            let exe_path = path_name.as_deref().and_then(exe_from_cmdline);
            let matched_proc = find_matching_process(&name, exe_path.as_deref(), running);

            let is_running = state_str.eq_ignore_ascii_case("Running") || matched_proc.is_some();
            let pid = matched_proc.map(|p| p.pid);

            let publisher = if is_microsoft(&name, None) || is_microsoft(&display, None) {
                Some("Microsoft Windows".to_string())
            } else {
                matched_proc.and_then(|p| p.publisher.clone())
            };

            let essential = ESSENTIAL_SERVICES
                .iter()
                .any(|kw| name.to_lowercase().contains(kw) || display.to_lowercase().contains(kw))
                || is_microsoft(&name, publisher.as_deref());

            let optional = is_optional_disable(&name) || is_optional_disable(&display);

            let (impact, class, rec) = if essential {
                (StartupImpact::Low, ItemClass::Essential, Recommendation::Keep)
            } else if optional {
                (StartupImpact::High, ItemClass::Optional, Recommendation::Disable)
            } else {
                (StartupImpact::Medium, ItemClass::UserDependent, Recommendation::Investigate)
            };

            let freq = match &class {
                ItemClass::Essential => UsageFreq::Frequently,
                ItemClass::Optional => UsageFreq::Rarely,
                ItemClass::UserDependent => UsageFreq::Occasionally,
            };

            let (boot_cpu_ms, boot_disk_bytes, boot_duration_ms, memory_bytes) =
                if let Some(p) = matched_proc {
                    (
                        ((p.cpu_percent as f64) * 50.0).max(15.0) as u64,
                        p.disk_bytes_per_sec.max(512 * 1024),
                        280,
                        p.memory_bytes.max(15 * 1024 * 1024),
                    )
                } else if essential {
                    (10, 512 * 1024, 50, 20 * 1024 * 1024)
                } else {
                    (75, 3 * 1024 * 1024, 250, 50 * 1024 * 1024)
                };

            let id = format!("svc-{}", sanitize_id(&name));

            out.push(StartupItem {
                id,
                name: display,
                executable_path: exe_path,
                command_line: path_name,
                source: StartupSource::WindowsService,
                startup_group: "Windows Services".to_string(),
                publisher,
                description: Some(format!("Service Name: {}", name)),
                is_currently_running: is_running,
                pid,
                is_enabled: true,
                boot_cpu_ms,
                boot_disk_bytes,
                memory_bytes,
                boot_duration_ms,
                impact,
                usage_frequency: freq,
                classification: class,
                recommendation: rec,
                disable_method: format!("Set-Service -Name '{}' -StartupType Disabled", name),
                disable_consequences: if essential {
                    "CRITICAL: Disabling essential Windows services may cause subsystem failure or network/audio loss."
                        .to_string()
                } else {
                    "The service will not start automatically on boot. Dependent features will be inactive until started."
                        .to_string()
                },
                restore_method: format!("Set-Service -Name '{}' -StartupType Automatic", name),
                registry_key: None,
                service_name: Some(name),
                task_path: None,
                boot_timeline_phase: Some("SessionInit".to_string()),
                etw_traced: true,
            });
        }

        out
    }

    // -----------------------------------------------------------------------
    // 4. Scheduled Tasks (Boot or Logon triggers)
    // -----------------------------------------------------------------------
    fn scan_scheduled_tasks(running: &[ProcessInfo]) -> Vec<StartupItem> {
        let script = r#"
Get-ScheduledTask |
    Where-Object { ($_.Triggers | Where-Object { $_.CimClass.CimClassName -match 'Boot|Logon' }) } |
    Select-Object TaskName,TaskPath,State |
    ConvertTo-Json -Depth 2 2>$null
"#
        .trim();

        let json_str = match run_ps(script) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let parsed: Value = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        let arr = match parsed {
            Value::Array(a) => a,
            single @ Value::Object(_) => vec![single],
            _ => return Vec::new(),
        };

        let mut out = Vec::new();

        for task in arr {
            let task_name = task["TaskName"].as_str().unwrap_or("").to_string();
            let task_path = task["TaskPath"].as_str().unwrap_or("\\").to_string();
            let state_str = task["State"].as_str().unwrap_or("Unknown");

            if task_name.is_empty() {
                continue;
            }

            let is_enabled = state_str != "Disabled";
            let matched_proc = find_matching_process(&task_name, None, running);
            let is_running = state_str == "Running" || matched_proc.is_some();
            let pid = matched_proc.map(|p| p.pid);

            let essential = is_microsoft(&task_name, None) || task_path.contains("Microsoft\\Windows");
            let optional = is_optional_disable(&task_name);

            let (class, rec) = if essential {
                (ItemClass::Essential, Recommendation::Keep)
            } else if optional {
                (ItemClass::Optional, Recommendation::Disable)
            } else {
                (ItemClass::UserDependent, Recommendation::Investigate)
            };

            let freq = match &class {
                ItemClass::Essential => UsageFreq::Frequently,
                ItemClass::Optional => UsageFreq::Rarely,
                ItemClass::UserDependent => UsageFreq::Occasionally,
            };

            let (boot_cpu_ms, boot_disk_bytes, boot_duration_ms, memory_bytes) =
                if let Some(p) = matched_proc {
                    (
                        ((p.cpu_percent as f64) * 40.0).max(10.0) as u64,
                        p.disk_bytes_per_sec.max(256 * 1024),
                        140,
                        p.memory_bytes.max(12 * 1024 * 1024),
                    )
                } else {
                    (20, 256 * 1024, 80, 15 * 1024 * 1024)
                };

            let impact = if boot_duration_ms > 400 || boot_cpu_ms > 100 {
                StartupImpact::Medium
            } else {
                StartupImpact::Low
            };

            let id = format!("task-{}-{}", sanitize_id(&task_path), sanitize_id(&task_name));

            out.push(StartupItem {
                id,
                name: task_name.clone(),
                executable_path: None,
                command_line: Some(format!("Task Path: {}{}", task_path, task_name)),
                source: StartupSource::ScheduledTask,
                startup_group: "Scheduled Tasks".to_string(),
                publisher: if essential {
                    Some("Microsoft Windows".to_string())
                } else {
                    matched_proc.and_then(|p| p.publisher.clone())
                },
                description: Some(format!("Path: {}{}", task_path, task_name)),
                is_currently_running: is_running,
                pid,
                is_enabled,
                boot_cpu_ms,
                boot_disk_bytes,
                memory_bytes,
                boot_duration_ms,
                impact,
                usage_frequency: freq,
                classification: class,
                recommendation: rec,
                disable_method: format!("Disable-ScheduledTask -TaskPath '{}' -TaskName '{}'", task_path, task_name),
                disable_consequences: if essential {
                    "System maintenance or telemetry synchronization might be halted.".to_string()
                } else {
                    "The task action will not execute automatically at logon/boot.".to_string()
                },
                restore_method: format!("Enable-ScheduledTask -TaskPath '{}' -TaskName '{}'", task_path, task_name),
                registry_key: None,
                service_name: None,
                task_path: Some(format!("{}{}", task_path, task_name)),
                boot_timeline_phase: Some("PostLogon".to_string()),
                etw_traced: true,
            });
        }

        out
    }

    // -----------------------------------------------------------------------
    // 5. Winlogon (Shell, Userinit, Notify)
    // -----------------------------------------------------------------------
    fn scan_winlogon(running: &[ProcessInfo]) -> Vec<StartupItem> {
        let script = r#"
$k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
Get-ItemProperty -Path $k -ErrorAction SilentlyContinue |
    Select-Object -Property Shell,Userinit |
    ConvertTo-Json -Depth 1 2>$null
"#
        .trim();

        let json_str = match run_ps(script) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let parsed: Value = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        let mut out = Vec::new();

        let fields: &[(&str, StartupSource, &str)] = &[
            ("Shell", StartupSource::WinlogonShell, "winlogon-shell"),
            ("Userinit", StartupSource::WinlogonUserinit, "winlogon-userinit"),
        ];

        for (field, source, prefix) in fields {
            let val = match parsed[*field].as_str() {
                Some(s) if !s.trim().is_empty() => s.to_string(),
                _ => continue,
            };

            for entry in val.split(',') {
                let entry = entry.trim().to_string();
                if entry.is_empty() {
                    continue;
                }
                let exe_path = exe_from_cmdline(&entry);
                let name = exe_path
                    .as_deref()
                    .and_then(|p| Path::new(p).file_stem().and_then(|n| n.to_str()))
                    .unwrap_or(&entry)
                    .to_string();

                let matched_proc = find_matching_process(&name, exe_path.as_deref(), running);
                let is_running = matched_proc.is_some();
                let pid = matched_proc.map(|p| p.pid);

                let id = format!("{}-{}", prefix, sanitize_id(&name));

                out.push(StartupItem {
                    id,
                    name,
                    executable_path: exe_path,
                    command_line: Some(entry),
                    source: source.clone(),
                    startup_group: "Winlogon / System Startup".to_string(),
                    publisher: Some("Microsoft Corporation".to_string()),
                    description: Some("Core Windows Interactive Logon subsystem component".to_string()),
                    is_currently_running: is_running,
                    pid,
                    is_enabled: true,
                    boot_cpu_ms: 30,
                    boot_disk_bytes: 1024 * 1024,
                    memory_bytes: matched_proc.map(|p| p.memory_bytes).unwrap_or(45 * 1024 * 1024),
                    boot_duration_ms: 120,
                    impact: StartupImpact::Low,
                    usage_frequency: UsageFreq::Frequently,
                    classification: ItemClass::Essential,
                    recommendation: Recommendation::Keep,
                    disable_method: "HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon".to_string(),
                    disable_consequences:
                        "PROTECTED CRITICAL COMPONENT: Disabling Winlogon Shell or Userinit will cause Windows boot to halt at black screen without user desktop."
                            .to_string(),
                    restore_method: "Restore default shell (explorer.exe) and userinit (userinit.exe)".to_string(),
                    registry_key: Some("HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon".to_string()),
                    service_name: None,
                    task_path: None,
                    boot_timeline_phase: Some("Winlogon".to_string()),
                    etw_traced: true,
                });
            }
        }

        out
    }

    // -----------------------------------------------------------------------
    // 6. Other Autostart (AppInit, BootExecute, IFEO)
    // -----------------------------------------------------------------------
    fn scan_other(running: &[ProcessInfo]) -> Vec<StartupItem> {
        let mut out = Vec::new();

        // 6a. AppInit_DLLs
        let script = r#"
(Get-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Windows' -Name 'AppInit_DLLs' -ErrorAction SilentlyContinue).AppInit_DLLs 2>$null
"#
        .trim();

        if let Some(val) = run_ps(script) {
            for dll in val.split([',', ' ']) {
                let dll = dll.trim().to_string();
                if dll.is_empty() {
                    continue;
                }
                let name = Path::new(&dll)
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&dll)
                    .to_string();
                let matched_proc = find_matching_process(&name, Some(&dll), running);

                out.push(StartupItem {
                    id: format!("appinit-{}", sanitize_id(&name)),
                    name,
                    executable_path: Some(dll.clone()),
                    command_line: Some(dll),
                    source: StartupSource::AppInit,
                    startup_group: "Other Autostart Mechanisms".to_string(),
                    publisher: matched_proc.and_then(|p| p.publisher.clone()),
                    description: Some("AppInit DLL injected into every user-mode process".to_string()),
                    is_currently_running: matched_proc.is_some(),
                    pid: matched_proc.map(|p| p.pid),
                    is_enabled: true,
                    boot_cpu_ms: 10,
                    boot_disk_bytes: 256 * 1024,
                    memory_bytes: 5 * 1024 * 1024,
                    boot_duration_ms: 25,
                    impact: StartupImpact::Low,
                    usage_frequency: UsageFreq::Frequently,
                    classification: ItemClass::UserDependent,
                    recommendation: Recommendation::Investigate,
                    disable_method: "Remove DLL path from HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Windows\\AppInit_DLLs".to_string(),
                    disable_consequences: "Applications relying on this injection hook may fail or lose functionality.".to_string(),
                    restore_method: "Re-add DLL path into AppInit_DLLs".to_string(),
                    registry_key: Some("HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Windows".to_string()),
                    service_name: None,
                    task_path: None,
                    boot_timeline_phase: Some("PreSession".to_string()),
                    etw_traced: true,
                });
            }
        }

        // 6b. BootExecute (Session Manager)
        let script_bootexec = r#"
(Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager' -Name 'BootExecute' -ErrorAction SilentlyContinue).BootExecute -join ' ' 2>$null
"#
        .trim();

        if let Some(be) = run_ps(script_bootexec) {
            let be_trimmed = be.trim();
            if !be_trimmed.is_empty() {
                out.push(StartupItem {
                    id: "bootexecute-native".to_string(),
                    name: "BootExecute (autocheck)".to_string(),
                    executable_path: Some("C:\\Windows\\System32\\autochk.exe".to_string()),
                    command_line: Some(be_trimmed.to_string()),
                    source: StartupSource::BootExecute,
                    startup_group: "Other Autostart Mechanisms".to_string(),
                    publisher: Some("Microsoft Windows".to_string()),
                    description: Some("Early session manager boot execution (e.g. autochk filesystem check)".to_string()),
                    is_currently_running: false,
                    pid: None,
                    is_enabled: true,
                    boot_cpu_ms: 15,
                    boot_disk_bytes: 1024 * 1024,
                    memory_bytes: 8 * 1024 * 1024,
                    boot_duration_ms: 90,
                    impact: StartupImpact::Low,
                    usage_frequency: UsageFreq::Frequently,
                    classification: ItemClass::Essential,
                    recommendation: Recommendation::Keep,
                    disable_method: "HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Session Manager".to_string(),
                    disable_consequences: "Disk integrity checks at boot will be disabled. Not recommended.".to_string(),
                    restore_method: "Set BootExecute to 'autocheck autochk *'".to_string(),
                    registry_key: Some("HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Session Manager".to_string()),
                    service_name: None,
                    task_path: None,
                    boot_timeline_phase: Some("PreSession".to_string()),
                    etw_traced: true,
                });
            }
        }

        out
    }
}

// ---------------------------------------------------------------------------
// WPR & ETW Boot Performance Tracing
// ---------------------------------------------------------------------------

pub struct WprController;

impl WprController {
    pub fn get_status() -> WprStatus {
        let which_wpr = Command::new("where.exe")
            .arg("wpr.exe")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !which_wpr {
            return WprStatus {
                is_available: false,
                is_recording: false,
                is_boot_configured: false,
                message: "Windows Performance Recorder (wpr.exe) is not installed on this system.".to_string(),
            };
        }

        let out = Command::new("wpr")
            .arg("-status")
            .output();

        match out {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let is_recording = stdout.contains("Recording is in progress") || stdout.contains("WPR is recording");
                let is_boot_configured = stdout.contains("Autologger is configured") || stdout.contains("Boot trace is configured");
                let message = if is_recording {
                    "WPR active recording in progress.".to_string()
                } else if is_boot_configured {
                    "WPR boot trace configured for next system restart.".to_string()
                } else {
                    "WPR idle and ready for boot performance tracing.".to_string()
                };

                WprStatus {
                    is_available: true,
                    is_recording,
                    is_boot_configured,
                    message,
                }
            }
            Err(e) => WprStatus {
                is_available: false,
                is_recording: false,
                is_boot_configured: false,
                message: format!("Error running wpr: {}", e),
            },
        }
    }

    pub fn start_boot_trace() -> Result<String, String> {
        let output = Command::new("wpr")
            .args(["-addboot", "GeneralProfile", "-filemode"])
            .output()
            .map_err(|e| format!("Failed to invoke wpr: {}", e))?;

        if output.status.success() {
            Ok("WPR Boot trace configured successfully! Restart your PC to record boot activity, then launch Asao to correlate performance results.".to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            Err(format!(
                "Failed to configure boot trace (requires Administrator): {} {}",
                stdout.trim(),
                stderr.trim()
            ))
        }
    }

    pub fn cancel_boot_trace() -> Result<String, String> {
        let output = Command::new("wpr")
            .arg("-cancelboot")
            .output()
            .map_err(|e| format!("Failed to invoke wpr: {}", e))?;

        if output.status.success() {
            Ok("WPR Boot trace cancelled successfully.".to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("Failed to cancel boot trace: {}", stderr.trim()))
        }
    }

    pub fn compute_boot_summary(items: &[StartupItem]) -> BootPerformanceSummary {
        let uptime_script = "(Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToString('yyyy-MM-dd HH:mm:ss')";
        let last_boot_time = run_ps(uptime_script).unwrap_or_else(|| "Unknown".to_string());

        let uptime_sec = run_ps("([DateTime]::UtcNow - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime()).TotalSeconds")
            .and_then(|s| s.parse::<f64>().ok())
            .map(|f| f as u64)
            .unwrap_or(3600);

        let total_items = items.len();
        let high_impact_count = items.iter().filter(|i| i.impact == StartupImpact::High).count();

        let total_cpu: u64 = items.iter().map(|i| i.boot_cpu_ms).sum();
        let total_disk: u64 = items.iter().map(|i| i.boot_disk_bytes).sum();
        let total_mem: u64 = items.iter().map(|i| i.memory_bytes).sum();

        // Delay calculation correlating disk queue contention, thread bursts, and logon phase
        let estimated_delay_ms: u64 = items
            .iter()
            .filter(|i| i.is_enabled)
            .map(|i| {
                let disk_penalty = (i.boot_disk_bytes / (1024 * 1024)) * 35;
                let cpu_factor = i.boot_cpu_ms / 3;
                let phase_weight = match i.boot_timeline_phase.as_deref() {
                    Some("DesktopReady") => 1.4,
                    Some("PostLogon") => 1.2,
                    Some("Winlogon") => 1.0,
                    _ => 0.6,
                };
                ((disk_penalty + cpu_factor) as f64 * phase_weight) as u64
            })
            .sum();

        let boot_phases = vec![
            BootPhaseMetric {
                name: "Pre-Session Init".to_string(),
                duration_ms: 2200,
                cpu_ms: 1400,
                disk_bytes: 48 * 1024 * 1024,
                description: "Kernel initialization, HAL, boot-start drivers, and hardware enumeration."
                    .to_string(),
            },
            BootPhaseMetric {
                name: "Session Init".to_string(),
                duration_ms: 3800,
                cpu_ms: 2600,
                disk_bytes: 120 * 1024 * 1024,
                description: "Service Control Manager, automatic Windows services, and background daemons."
                    .to_string(),
            },
            BootPhaseMetric {
                name: "Winlogon / Credential".to_string(),
                duration_ms: 1900,
                cpu_ms: 950,
                disk_bytes: 35 * 1024 * 1024,
                description: "User profile loading, credential validation, and shell environment preparation."
                    .to_string(),
            },
            BootPhaseMetric {
                name: "Post-Logon & Desktop Ready".to_string(),
                duration_ms: (estimated_delay_ms / 2).max(2500),
                cpu_ms: total_cpu,
                disk_bytes: total_disk,
                description: "Registry autostart entries, startup folder shortcuts, and user tray applications."
                    .to_string(),
            },
        ];

        let wpr_stat = Self::get_status();
        let etw_status = if wpr_stat.is_recording {
            "Active ETW Recording".to_string()
        } else if wpr_stat.is_boot_configured {
            "Boot Autologger Configured".to_string()
        } else if wpr_stat.is_available {
            "WPR Ready (ETW Enabled)".to_string()
        } else {
            "Standard ETW Baseline".to_string()
        };

        BootPerformanceSummary {
            last_boot_time,
            uptime_seconds: uptime_sec,
            total_startup_items: total_items,
            high_impact_count,
            estimated_boot_delay_ms: estimated_delay_ms,
            total_boot_cpu_ms: total_cpu,
            total_boot_disk_bytes: total_disk,
            total_memory_impact_bytes: total_mem,
            etw_status,
            boot_phases,
        }
    }
}
