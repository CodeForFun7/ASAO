//! `ServiceInventory`, `ScheduledTaskInventory`, `StartupInventory` and device facts.
//! Apps come from the leftover cleanup's inventory (`residual::inventory`). Read-only.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{AntivirusProduct, DeviceContext, ServiceInfo, StartupEntryInfo, TaskInfo};
use crate::analyzer::ProcessInfo;
use crate::residual::normalize::{is_same_or_inside, path_key};
use crate::startup::scanner::{run_ps, StartupScanner, StartupSource};

fn as_array(v: Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a,
        Value::Null => Vec::new(),
        other => vec![other],
    }
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn strings_of(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .map(|x| as_array(x.clone()))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| s.as_str().map(|s| s.to_string()))
        .collect()
}

/// The program a command line runs, e.g. `"C:\Foo\foo.exe" /bar` → `C:\Foo\foo.exe`.
pub fn exe_of_command(cmd: &str) -> Option<String> {
    let c = cmd.trim();
    if let Some(rest) = c.strip_prefix('"') {
        return rest.split('"').next().map(|s| s.to_string()).filter(|s| !s.is_empty());
    }
    let lower = c.to_lowercase();
    if let Some(i) = lower.find(".exe") {
        return Some(c[..i + 4].to_string());
    }
    c.split_whitespace().next().map(|s| s.to_string())
}

fn is_windows_path(path: &str, windir: Option<&str>) -> bool {
    let k = path_key(path);
    windir.map(|w| is_same_or_inside(&k, w)).unwrap_or(false)
}

/// Services (no kernel or file-system drivers: `Win32_Service` lists none) with their
/// dependencies in both directions.
pub fn collect_services() -> Option<Vec<ServiceInfo>> {
    let script = r#"
$s = Get-CimInstance Win32_Service | Select-Object Name,DisplayName,Description,PathName,StartMode,State,StartName,ProcessId,DelayedAutoStart
$d = Get-Service -ErrorAction SilentlyContinue | ForEach-Object {
  [pscustomobject]@{ N = $_.Name; On = @($_.ServicesDependedOn | ForEach-Object Name); By = @($_.DependentServices | ForEach-Object Name) }
}
@{ s = @($s); d = @($d) } | ConvertTo-Json -Depth 4 -Compress
"#;
    let out = run_ps(script.trim())?;
    let v: Value = serde_json::from_str(out.trim_start_matches('\u{feff}')).ok()?;
    let windir = std::env::var("WINDIR").ok().map(|w| path_key(&w));
    let deps: Vec<(String, Vec<String>, Vec<String>)> = as_array(v["d"].clone())
        .iter()
        .filter_map(|d| Some((str_of(d, "N")?, strings_of(d, "On"), strings_of(d, "By"))))
        .collect();
    let mut out = Vec::new();
    for s in as_array(v["s"].clone()) {
        let Some(name) = str_of(&s, "Name") else { continue };
        let path = str_of(&s, "PathName");
        let exe = path.as_deref().and_then(exe_of_command);
        let (on, by) = deps
            .iter()
            .find(|d| d.0.eq_ignore_ascii_case(&name))
            .map(|d| (d.1.clone(), d.2.clone()))
            .unwrap_or_default();
        out.push(ServiceInfo {
            display_name: str_of(&s, "DisplayName").unwrap_or_else(|| name.clone()),
            description: str_of(&s, "Description"),
            is_windows: exe.as_deref().map(|e| is_windows_path(e, windir.as_deref())).unwrap_or(true),
            binary_path: path,
            start_type: str_of(&s, "StartMode").unwrap_or_else(|| "Unknown".into()),
            delayed_auto_start: s["DelayedAutoStart"].as_bool().unwrap_or(false),
            state: str_of(&s, "State").unwrap_or_else(|| "Unknown".into()),
            account: str_of(&s, "StartName"),
            process_id: s["ProcessId"].as_u64().unwrap_or(0) as u32,
            depends_on: on,
            dependents: by,
            planned_start_type: "Manual".into(),
            name,
        });
    }
    Some(out)
}

/// ISO 8601 duration ("PT1H", "PT30M", "P1D") in seconds.
pub fn iso_duration_secs(s: &str) -> Option<u64> {
    let s = s.trim();
    let body = s.strip_prefix('P')?;
    let (date, time) = match body.find('T') {
        Some(i) => (&body[..i], &body[i + 1..]),
        None => (body, ""),
    };
    let mut total = 0u64;
    let mut num = String::new();
    for (part, is_time) in [(date, false), (time, true)] {
        for ch in part.chars() {
            if ch.is_ascii_digit() {
                num.push(ch);
                continue;
            }
            let n: u64 = num.parse().ok()?;
            num.clear();
            total += n * match (ch, is_time) {
                ('D', false) => 86_400,
                ('W', false) => 7 * 86_400,
                ('H', true) => 3_600,
                ('M', true) => 60,
                ('S', true) => 1,
                _ => return None,
            };
        }
    }
    Some(total)
}

/// Turns one trigger into plain words, and says whether it runs at logon/boot or more
/// often than daily.
pub fn describe_trigger(class: &str, repetition: Option<&str>, days_interval: Option<u64>) -> (String, bool) {
    let rep = repetition.and_then(iso_duration_secs).filter(|s| *s > 0);
    let base = if class.contains("Logon") {
        ("At sign-in".to_string(), true)
    } else if class.contains("Boot") {
        ("At startup".to_string(), true)
    } else if class.contains("Idle") {
        ("When the PC is idle".to_string(), true)
    } else if class.contains("Daily") {
        let d = days_interval.unwrap_or(1);
        (if d <= 1 { "Daily".to_string() } else { format!("Every {} days", d) }, false)
    } else if class.contains("Weekly") {
        ("Weekly".to_string(), false)
    } else if class.contains("Time") {
        ("At a set time".to_string(), false)
    } else if class.contains("Event") {
        ("When a system event happens".to_string(), false)
    } else {
        ("On a schedule".to_string(), false)
    };
    match rep {
        Some(secs) if secs < 86_400 => {
            let every = if secs % 3_600 == 0 {
                format!("every {} h", secs / 3_600)
            } else {
                format!("every {} min", (secs / 60).max(1))
            };
            (format!("{}, then {}", base.0, every), true)
        }
        _ => base,
    }
}

/// Scheduled tasks outside `\Microsoft\` (those are Windows' own).
pub fn collect_tasks() -> Option<Vec<TaskInfo>> {
    let script = r#"
Get-ScheduledTask -ErrorAction SilentlyContinue | Where-Object { $_.TaskPath -notlike '\Microsoft\*' } | ForEach-Object {
  [pscustomobject]@{
    P = $_.TaskPath; N = $_.TaskName; S = [string]$_.State
    A = @($_.Actions | ForEach-Object { ("$($_.Execute) $($_.Arguments)").Trim() })
    T = @($_.Triggers | ForEach-Object { [pscustomobject]@{ C = $_.CimClass.CimClassName; R = $_.Repetition.Interval; D = $_.DaysInterval; E = $_.Enabled } })
  }
} | ConvertTo-Json -Depth 4 -Compress
"#;
    let out = run_ps(script.trim()).unwrap_or_else(|| "[]".into());
    let v: Value = serde_json::from_str(out.trim_start_matches('\u{feff}')).ok()?;
    let mut tasks = Vec::new();
    for t in as_array(v) {
        let Some(name) = str_of(&t, "N") else { continue };
        let folder = str_of(&t, "P").unwrap_or_else(|| "\\".into());
        let mut frequent = false;
        let mut triggers = Vec::new();
        for tr in as_array(t["T"].clone()) {
            if tr["E"].as_bool() == Some(false) {
                continue;
            }
            let class = str_of(&tr, "C").unwrap_or_default();
            let (text, freq) = describe_trigger(&class, str_of(&tr, "R").as_deref(), tr["D"].as_u64());
            frequent |= freq;
            triggers.push(text);
        }
        tasks.push(TaskInfo {
            path: format!("{}{}", folder, name),
            action: strings_of(&t, "A").into_iter().next(),
            enabled: !str_of(&t, "S").map(|s| s.eq_ignore_ascii_case("Disabled")).unwrap_or(false),
            frequent,
            triggers,
            name,
        });
    }
    Some(tasks)
}

/// Run keys and Startup folders, from the Startup page's scanner (it reads StartupApproved,
/// so entries already disabled in Task Manager show as disabled).
pub fn collect_startup(running: &[ProcessInfo]) -> Vec<StartupEntryInfo> {
    let mut items = StartupScanner::scan_registry(running);
    items.extend(StartupScanner::scan_startup_folders(running));
    items
        .into_iter()
        .filter(|i| {
            matches!(
                i.source,
                StartupSource::RegistryRunUser
                    | StartupSource::RegistryRunMachine
                    | StartupSource::StartupFolderUser
                    | StartupSource::StartupFolderMachine
            )
        })
        .map(|i| StartupEntryInfo {
            source: serde_json::to_value(&i.source)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default(),
            executable: i
                .executable_path
                .clone()
                .or_else(|| i.command_line.as_deref().and_then(exe_of_command)),
            command: i.command_line,
            enabled: i.is_enabled,
            registry_key: i.registry_key,
            boot_cpu_ms: None,
            boot_disk_bytes: None,
            id: i.id,
            name: i.name,
        })
        .collect()
}

/// Steam and Epic library manifests, or the Gaming Services package (Xbox / Game Pass).
pub fn games_installed(steam_path: Option<&str>, has_gaming_services: bool) -> bool {
    if has_gaming_services {
        return true;
    }
    let mut steam_roots: Vec<PathBuf> = Vec::new();
    if let Some(p) = steam_path {
        steam_roots.push(PathBuf::from(p.replace('/', "\\")));
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(pf) = std::env::var(var) {
            steam_roots.push(Path::new(&pf).join("Steam"));
        }
    }
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in &steam_roots {
        libraries.push(root.clone());
        if let Ok(vdf) = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf")) {
            for line in vdf.lines() {
                let l = line.trim();
                if let Some(rest) = l.strip_prefix("\"path\"") {
                    let p = rest.trim().trim_matches('"').replace("\\\\", "\\");
                    libraries.push(PathBuf::from(p));
                }
            }
        }
    }
    let has_manifest = |dir: &Path, prefix: &str, ext: &str| {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten().any(|e| {
                    let n = e.file_name().to_string_lossy().to_lowercase();
                    n.starts_with(prefix) && n.ends_with(ext)
                })
            })
            .unwrap_or(false)
    };
    if libraries
        .iter()
        .any(|lib| has_manifest(&lib.join("steamapps"), "appmanifest_", ".acf"))
    {
        return true;
    }
    if let Ok(pd) = std::env::var("ProgramData") {
        let epic = Path::new(&pd).join("Epic").join("EpicGamesLauncher").join("Data").join("Manifests");
        if has_manifest(&epic, "", ".item") {
            return true;
        }
    }
    false
}

/// Maker, model, organisation management, antivirus and the Steam folder.
pub fn collect_device(has_gaming_services: bool) -> DeviceContext {
    let script = r#"
$cs = Get-CimInstance Win32_ComputerSystem -ErrorAction SilentlyContinue
$av = @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ N = $_.displayName; S = [int64]$_.productState } })
$mdm = @(Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Enrollments' -ErrorAction SilentlyContinue | ForEach-Object { Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue } | Where-Object { $_.ProviderID -and ($_.UPN -or $_.DiscoveryServiceFullURL) } | ForEach-Object { $_.ProviderID })
$entra = @(Get-ChildItem 'HKLM:\SYSTEM\CurrentControlSet\Control\CloudDomainJoin\JoinInfo' -ErrorAction SilentlyContinue).Count -gt 0
$steam = (Get-ItemProperty 'HKCU:\Software\Valve\Steam' -ErrorAction SilentlyContinue).SteamPath
$build = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -ErrorAction SilentlyContinue).CurrentBuildNumber
[pscustomobject]@{ M = $cs.Manufacturer; Mo = $cs.Model; D = [bool]$cs.PartOfDomain; Dn = $cs.Domain; Av = $av; Mdm = $mdm; Entra = $entra; Steam = $steam; Build = [string]$build } | ConvertTo-Json -Depth 4 -Compress
"#;
    let v: Value = run_ps(script.trim())
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
        .unwrap_or(Value::Null);
    let antivirus: Vec<AntivirusProduct> = as_array(v["Av"].clone())
        .iter()
        .filter_map(|a| {
            let name = str_of(a, "N")?;
            // productState: bits 12-15 hold the on/off state (1 = on).
            let state = a["S"].as_i64().unwrap_or(0);
            Some(AntivirusProduct { name, enabled: (state >> 12) & 0xF == 1 })
        })
        .collect();
    let defender_present = antivirus.iter().any(|a| {
        let n = a.name.to_lowercase();
        n.contains("windows defender") || n.contains("microsoft defender")
    });
    let domain = v["D"].as_bool().unwrap_or(false);
    let mdm = strings_of(&v, "Mdm");
    let managed_reason = if domain {
        Some(format!(
            "Joined to the domain {}",
            str_of(&v, "Dn").unwrap_or_else(|| "of an organisation".into())
        ))
    } else if !mdm.is_empty() {
        Some(format!("Enrolled in device management ({})", mdm.join(", ")))
    } else if v["Entra"].as_bool().unwrap_or(false) {
        Some("Joined to an organisation's Microsoft Entra ID".into())
    } else {
        None
    };
    DeviceContext {
        manufacturer: str_of(&v, "M"),
        model: str_of(&v, "Mo"),
        org_managed: managed_reason.is_some(),
        managed_reason,
        games_installed: games_installed(str_of(&v, "Steam").as_deref(), has_gaming_services),
        antivirus,
        defender_present,
        is_elevated: crate::residual::elevated::is_process_elevated(),
        startup_measured: false,
        windows_build: str_of(&v, "Build").and_then(|b| b.parse().ok()),
    }
}
