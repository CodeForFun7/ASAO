//! `ChangeExecutor`: runs a plan in order, records each original state for undo, and logs
//! every action. Services are never deleted (only stopped and set to Manual or Disabled),
//! startup entries and tasks are disabled (never deleted), and apps are removed only through
//! Windows (Store) or their own uninstaller (Win32). Dry run reads the original states and
//! logs what would happen, changing nothing.

use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::planner::{Action, ActionKind};
use crate::residual::now_ms;
use crate::residual::store_files::append_jsonl;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutcomeStatus {
    Done,
    WouldDo,
    Skipped,
    Failed,
    /// Windows refused: the service or package is protected.
    Protected,
    /// The uninstaller finished, but the app is still listed.
    NotConfirmed,
}

/// What a change replaced, so it can be put back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum Original {
    /// StartupApproved values before the change; `None` bytes means the value did not exist.
    StartupApproved { key: String, values: Vec<(String, Option<String>)> },
    Task { enabled: bool },
    Service { start_type: String, delayed: bool, running: bool },
    /// What a tweak's registry file replaced, value by value.
    Registry { backups: Vec<super::regfile::RegBackup> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub action: Action,
    pub status: OutcomeStatus,
    pub message: Option<String>,
    pub original: Option<Original>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestorePointPolicy {
    /// Stop before changing anything when no restore point can be made.
    Stop,
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestorePointStatus {
    Created,
    /// Windows made one in the last 24 hours and does not make another; that one is the fallback.
    Recent,
    Unavailable,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePointResult {
    pub status: RestorePointStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ExecOptions<'a> {
    pub dry_run: bool,
    pub restore_point_policy: RestorePointPolicy,
    pub elevated: bool,
    /// Where tweaks' registry files come from.
    pub w11d: Option<&'a super::w11d::Win11Debloat>,
    /// Scratch folder for registry files being imported or exported.
    pub scratch: &'a Path,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecReport {
    pub restore_point: RestorePointResult,
    pub outcomes: Vec<Outcome>,
    /// The run stopped before changing anything (no restore point, and the user chose Stop).
    pub stopped: bool,
    /// A changed setting needs a restart (Win11Debloat's RequiresReboot).
    #[serde(default)]
    pub restart_needed: bool,
}

/// Single-quoted PowerShell string.
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Runs a PowerShell script and returns its output. Fails on a non-zero exit.
pub fn ps(script: &str) -> Result<String, String> {
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| format!("Could not start PowerShell: {}", e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().trim_start_matches('\u{feff}').to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(first_line(if err.is_empty() { &stdout } else { &err }))
    }
}

fn first_line(s: &str) -> String {
    let line = s.lines().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or("Windows reported an error");
    line.trim_start_matches("ERR:").trim().to_string()
}

fn is_access_denied(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("access is denied") || m.contains("access denied") || m.contains("0x80070005")
}

// ---- Startup entries (StartupApproved, the same switch Task Manager uses) ---------------------

/// The StartupApproved key and value names for an entry.
pub fn approved_target(name: &str, source: &str, registry_key: Option<&str>) -> (String, Vec<String>) {
    let machine = source.ends_with("-machine");
    let hive = if machine { "HKLM" } else { "HKCU" };
    if source.starts_with("startup-folder") {
        let lnk = if name.to_lowercase().ends_with(".lnk") { name.to_string() } else { format!("{}.lnk", name) };
        (
            format!(r"{}:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder", hive),
            vec![lnk],
        )
    } else {
        let wow = registry_key.map(|k| k.to_lowercase().contains("wow6432node")).unwrap_or(false);
        (
            format!(
                r"{}:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\{}",
                hive,
                if wow { "Run32" } else { "Run" }
            ),
            vec![name.to_string()],
        )
    }
}

fn read_approved(key: &str, names: &[String]) -> Result<Vec<(String, Option<String>)>, String> {
    let mut out = Vec::new();
    for n in names {
        let script = format!(
            "$v = (Get-ItemProperty -Path {k} -Name {n} -ErrorAction SilentlyContinue).{n}; if ($v) {{ [Convert]::ToBase64String([byte[]]$v) }}",
            k = ps_quote(key),
            n = ps_quote(n)
        );
        let v = ps(&script)?;
        out.push((n.clone(), if v.is_empty() { None } else { Some(v) }));
    }
    Ok(out)
}

/// Writes 0x03 (disabled) or 0x02 (enabled) followed by eleven zero bytes, as Task Manager does.
fn set_approved(key: &str, name: &str, enabled: bool) -> Result<(), String> {
    let first = if enabled { "0x02" } else { "0x03" };
    ps(&format!(
        "if (-not (Test-Path {k})) {{ New-Item -Path {k} -Force | Out-Null }}; Set-ItemProperty -Path {k} -Name {n} -Type Binary -Value ([byte[]]@({f},0,0,0,0,0,0,0,0,0,0,0)) -ErrorAction Stop",
        k = ps_quote(key),
        n = ps_quote(name),
        f = first
    ))
    .map(|_| ())
}

/// Puts StartupApproved values back exactly as they were.
pub fn restore_approved(key: &str, values: &[(String, Option<String>)]) -> Result<(), String> {
    for (name, bytes) in values {
        let script = match bytes {
            Some(b64) => format!(
                "Set-ItemProperty -Path {k} -Name {n} -Type Binary -Value ([Convert]::FromBase64String({b})) -ErrorAction Stop",
                k = ps_quote(key),
                n = ps_quote(name),
                b = ps_quote(b64)
            ),
            None => format!(
                "Remove-ItemProperty -Path {k} -Name {n} -ErrorAction SilentlyContinue",
                k = ps_quote(key),
                n = ps_quote(name)
            ),
        };
        ps(&script)?;
    }
    Ok(())
}

// ---- Scheduled tasks -------------------------------------------------------------------------

/// `\Folder\Sub\Name` → (`\Folder\Sub\`, `Name`).
pub fn split_task_path(path: &str) -> (String, String) {
    match path.rfind('\\') {
        Some(i) => (path[..=i].to_string(), path[i + 1..].to_string()),
        None => ("\\".to_string(), path.to_string()),
    }
}

fn task_enabled(path: &str) -> Result<bool, String> {
    let (folder, name) = split_task_path(path);
    let state = ps(&format!(
        "(Get-ScheduledTask -TaskPath {} -TaskName {} -ErrorAction Stop).State",
        ps_quote(&folder),
        ps_quote(&name)
    ))?;
    Ok(!state.eq_ignore_ascii_case("Disabled"))
}

pub fn set_task(path: &str, enabled: bool) -> Result<(), String> {
    let (folder, name) = split_task_path(path);
    ps(&format!(
        "{} -TaskPath {} -TaskName {} -ErrorAction Stop | Out-Null",
        if enabled { "Enable-ScheduledTask" } else { "Disable-ScheduledTask" },
        ps_quote(&folder),
        ps_quote(&name)
    ))
    .map(|_| ())
}

// ---- Services --------------------------------------------------------------------------------

fn service_state(name: &str) -> Result<Original, String> {
    let out = ps(&format!(
        "$s = Get-CimInstance Win32_Service -Filter (\"Name='\" + {} + \"'\") -ErrorAction Stop; if (-not $s) {{ throw 'Service not found' }}; [pscustomobject]@{{ M = $s.StartMode; D = [bool]$s.DelayedAutoStart; S = $s.State }} | ConvertTo-Json -Compress",
        ps_quote(&name.replace('\'', ""))
    ))?;
    let v: Value = serde_json::from_str(&out).map_err(|_| "Could not read the service".to_string())?;
    Ok(Original::Service {
        start_type: v["M"].as_str().unwrap_or("Manual").to_string(),
        delayed: v["D"].as_bool().unwrap_or(false),
        running: v["S"].as_str().map(|s| s.eq_ignore_ascii_case("Running")).unwrap_or(false),
    })
}

/// Windows' StartMode ("Auto") to Set-Service's StartupType ("Automatic").
fn startup_type_arg(mode: &str) -> &'static str {
    match mode.to_lowercase().as_str() {
        "auto" | "automatic" => "Automatic",
        "disabled" => "Disabled",
        _ => "Manual",
    }
}

fn configure_service(name: &str, start_type: &str, stop: bool) -> Result<(), String> {
    ps(&format!(
        "Set-Service -Name {} -StartupType {} -ErrorAction Stop",
        ps_quote(name),
        startup_type_arg(start_type)
    ))?;
    if stop {
        // No -Force: a service other services depend on is not stopped.
        ps(&format!("Stop-Service -Name {} -ErrorAction Stop", ps_quote(name)))?;
    }
    Ok(())
}

pub fn restore_service(name: &str, start_type: &str, delayed: bool, running: bool) -> Result<(), String> {
    ps(&format!(
        "Set-Service -Name {} -StartupType {} -ErrorAction Stop",
        ps_quote(name),
        startup_type_arg(start_type)
    ))?;
    if delayed && startup_type_arg(start_type) == "Automatic" {
        ps(&format!("sc.exe config {} start= delayed-auto | Out-Null", ps_quote(name)))?;
    }
    if running {
        ps(&format!("Start-Service -Name {} -ErrorAction Stop", ps_quote(name)))?;
    }
    Ok(())
}

// ---- Apps ------------------------------------------------------------------------------------

fn store_installed(package_name: &str) -> bool {
    ps(&format!("@(Get-AppxPackage -Name {}).Count", ps_quote(package_name)))
        .map(|c| c.trim() != "0")
        .unwrap_or(true)
}

fn uninstall_store(full_name: &str, package_name: &str) -> Result<OutcomeStatus, String> {
    crate::apps::actions::remove_store_package(full_name)?;
    Ok(if store_installed(package_name) { OutcomeStatus::NotConfirmed } else { OutcomeStatus::Done })
}

fn remove_provisioned(package_name: &str) -> Result<OutcomeStatus, String> {
    let removed = ps(&format!(
        "$p = @(Get-AppxProvisionedPackage -Online -ErrorAction Stop | Where-Object DisplayName -eq {}); foreach ($x in $p) {{ Remove-AppxProvisionedPackage -Online -PackageName $x.PackageName -ErrorAction Stop | Out-Null }}; $p.Count",
        ps_quote(package_name)
    ))?;
    if removed.trim() == "0" {
        return Err("It is not provisioned for new users, so there is nothing to remove.".into());
    }
    Ok(OutcomeStatus::Done)
}

const UNINSTALL_KEYS: &str = "'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall','HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall','HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall'";

/// The uninstall entry for an app: (quiet command, normal command), or `None` when gone.
fn find_uninstaller(display_name: &str, subkey: Option<&str>) -> Result<Option<(Option<String>, Option<String>)>, String> {
    let filter = match subkey {
        Some(k) => format!("$_.PSChildName -eq {}", ps_quote(k)),
        None => format!("$_.DisplayName -eq {}", ps_quote(display_name)),
    };
    let out = ps(&format!(
        "$e = foreach ($r in @({keys})) {{ Get-ChildItem $r -ErrorAction SilentlyContinue | ForEach-Object {{ Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue }} | Where-Object {{ {filter} }} }}; $e = @($e) | Select-Object -First 1; if ($e) {{ [pscustomobject]@{{ Q = $e.QuietUninstallString; U = $e.UninstallString }} | ConvertTo-Json -Compress }}",
        keys = UNINSTALL_KEYS,
        filter = filter
    ))?;
    if out.is_empty() {
        return Ok(None);
    }
    let v: Value = serde_json::from_str(&out).map_err(|_| "Could not read the uninstall entry".to_string())?;
    let s = |k: &str| v[k].as_str().map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    Ok(Some((s("Q"), s("U"))))
}

fn uninstall_win32(display_name: &str, subkey: Option<&str>) -> Result<OutcomeStatus, String> {
    let Some((quiet, normal)) = find_uninstaller(display_name, subkey)? else {
        return Ok(OutcomeStatus::Done);
    };
    let command = quiet.or(normal).ok_or_else(|| "It has no uninstaller registered.".to_string())?;
    let (file, args) = crate::apps::actions::split_command(&command)
        .ok_or_else(|| "Its uninstall command could not be read.".to_string())?;
    let mut cmd = std::process::Command::new(&file);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if !args.is_empty() {
            cmd.raw_arg(&args);
        }
    }
    // The uninstaller's own window may appear. ASAO waits for it, one at a time.
    let status = cmd.status().map_err(|e| format!("Could not start the uninstaller: {}", e))?;
    // Some uninstallers hand over to a copy of themselves and exit at once: watch the
    // registry for up to two minutes.
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(120) {
        if find_uninstaller(display_name, subkey)?.is_none() {
            return Ok(OutcomeStatus::Done);
        }
        std::thread::sleep(Duration::from_secs(3));
    }
    if !status.success() {
        return Err(format!("The uninstaller ended with code {}", status.code().unwrap_or(-1)));
    }
    Ok(OutcomeStatus::NotConfirmed)
}

// ---- Restore point ---------------------------------------------------------------------------

pub fn create_restore_point() -> RestorePointResult {
    let script = r#"
$before = @(Get-ComputerRestorePoint -ErrorAction SilentlyContinue).Count
try { Checkpoint-Computer -Description 'ASAO: before removing bloatware' -RestorePointType MODIFY_SETTINGS -ErrorAction Stop -WarningAction SilentlyContinue }
catch { 'ERR:' + $_.Exception.Message; exit 0 }
$all = @(Get-ComputerRestorePoint -ErrorAction SilentlyContinue)
if ($all.Count -gt $before) { 'CREATED'; exit 0 }
$last = $all | Sort-Object SequenceNumber | Select-Object -Last 1
if ($last) { 'RECENT:' + $last.ConvertToDateTime($last.CreationTime).ToUniversalTime().ToString('o') } else { 'NONE' }
"#;
    match ps(script.trim()) {
        Ok(out) if out.starts_with("CREATED") => RestorePointResult {
            status: RestorePointStatus::Created,
            message: "Created a System Restore point.".into(),
        },
        Ok(out) if out.starts_with("RECENT:") => {
            let when = out.trim_start_matches("RECENT:").to_string();
            let recent = chrono_age_hours(&when).map(|h| h < 24.0).unwrap_or(false);
            if recent {
                RestorePointResult {
                    status: RestorePointStatus::Recent,
                    message: format!(
                        "Windows makes one restore point a day and already made one at {}. That one is your fallback.",
                        when
                    ),
                }
            } else {
                RestorePointResult {
                    status: RestorePointStatus::Unavailable,
                    message: "Windows did not create a restore point.".into(),
                }
            }
        }
        Ok(out) if out.starts_with("ERR:") => RestorePointResult {
            status: RestorePointStatus::Unavailable,
            message: format!(
                "No restore point: {}. System Protection may be off (Control Panel, System, System Protection).",
                out.trim_start_matches("ERR:").trim().trim_end_matches('.')
            ),
        },
        Ok(_) => RestorePointResult {
            status: RestorePointStatus::Unavailable,
            message: "No restore point could be made. System Protection may be off.".into(),
        },
        Err(e) => RestorePointResult {
            status: RestorePointStatus::Unavailable,
            message: format!("No restore point could be made: {}", e),
        },
    }
}

/// Hours since an ISO 8601 UTC time ("2026-10-31T09:12:00.0000000Z").
fn chrono_age_hours(iso: &str) -> Option<f64> {
    let t = iso.trim();
    let (date, rest) = t.split_once('T')?;
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time: String = rest.chars().take(8).collect();
    let mut tp = time.split(':').map(|x| x.parse::<i64>().ok());
    let (hh, mm, ss) = (tp.next()??, tp.next()??, tp.next()??);
    // Days from civil (Howard Hinnant).
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let ms = (days * 86_400 + hh * 3_600 + mm * 60 + ss) * 1000;
    Some((now_ms() as i64 - ms) as f64 / 3_600_000.0)
}

// ---- Running a plan --------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogLine<'a> {
    timestamp_ms: u64,
    batch_id: &'a str,
    dry_run: bool,
    elevated: bool,
    item: &'a str,
    action: &'a ActionKind,
    status: OutcomeStatus,
    message: Option<&'a str>,
    original: Option<&'a Original>,
}

fn log(path: &Path, batch_id: &str, opts: &ExecOptions, o: &Outcome) {
    append_jsonl(
        path,
        &LogLine {
            timestamp_ms: now_ms(),
            batch_id,
            dry_run: opts.dry_run,
            elevated: opts.elevated,
            item: &o.action.item_name,
            action: &o.action.kind,
            status: o.status,
            message: o.message.as_deref(),
            original: o.original.as_ref(),
        },
    );
}

/// Reads the state a change would replace. Read-only.
fn read_original(kind: &ActionKind, opts: &ExecOptions) -> Result<Option<Original>, String> {
    Ok(match kind {
        ActionKind::ApplyTweak { feature_id, .. } => {
            let (_, ops, _) = opts
                .w11d
                .and_then(|w| w.ops_for(feature_id))
                .ok_or_else(|| "Its registry file is missing.".to_string())?;
            Some(Original::Registry { backups: super::regfile::backup(&ops, opts.scratch)? })
        }
        ActionKind::DisableStartup { name, source, registry_key } => {
            let (key, names) = approved_target(name, source, registry_key.as_deref());
            Some(Original::StartupApproved { values: read_approved(&key, &names)?, key })
        }
        ActionKind::DisableTask { path } => Some(Original::Task { enabled: task_enabled(path)? }),
        ActionKind::ConfigureService { name, .. } => Some(service_state(name)?),
        _ => None,
    })
}

fn apply(kind: &ActionKind, opts: &ExecOptions) -> Result<OutcomeStatus, String> {
    match kind {
        ActionKind::ApplyTweak { feature_id, .. } => {
            let (_, ops, bytes) = opts
                .w11d
                .and_then(|w| w.ops_for(feature_id))
                .ok_or_else(|| "Its registry file is missing.".to_string())?;
            super::regfile::import(&bytes, opts.scratch)?;
            let live = super::regfile::read_live(&super::regfile::queries_for(&ops))?;
            Ok(if super::regfile::all_match(&ops, &live) { OutcomeStatus::Done } else { OutcomeStatus::NotConfirmed })
        }
        ActionKind::RestorePoint => Ok(OutcomeStatus::Done),
        ActionKind::DisableStartup { name, source, registry_key } => {
            let (key, names) = approved_target(name, source, registry_key.as_deref());
            for n in &names {
                set_approved(&key, n, false)?;
            }
            Ok(OutcomeStatus::Done)
        }
        ActionKind::DisableTask { path } => set_task(path, false).map(|_| OutcomeStatus::Done),
        ActionKind::ConfigureService { name, start_type, stop, .. } => {
            configure_service(name, start_type, *stop).map(|_| OutcomeStatus::Done)
        }
        ActionKind::UninstallStore { package_full_name, package_name } => uninstall_store(package_full_name, package_name),
        ActionKind::RemoveProvisioned { package_name } => remove_provisioned(package_name),
        ActionKind::UninstallWin32 { display_name, registry_subkey } => {
            uninstall_win32(display_name, registry_subkey.as_deref())
        }
    }
}

/// Runs every action in order, carrying on past failures.
pub fn execute(
    actions: &[Action],
    opts: ExecOptions,
    batch_id: &str,
    log_path: &Path,
    progress: &mut dyn FnMut(usize, usize, &str),
) -> ExecReport {
    let mut outcomes = Vec::new();
    let mut restore = RestorePointResult {
        status: RestorePointStatus::Skipped,
        message: if opts.dry_run {
            "Dry run: no restore point was made.".into()
        } else {
            "No changes were planned.".into()
        },
    };
    let total = actions.len();
    for (i, a) in actions.iter().enumerate() {
        progress(i, total, &a.description);
        if matches!(a.kind, ActionKind::RestorePoint) {
            if !opts.dry_run {
                restore = create_restore_point();
                let ok = matches!(restore.status, RestorePointStatus::Created | RestorePointStatus::Recent);
                if !ok && opts.restore_point_policy == RestorePointPolicy::Stop {
                    for rest in actions.iter().skip(i + 1) {
                        let o = Outcome {
                            action: rest.clone(),
                            status: OutcomeStatus::Skipped,
                            message: Some("Stopped: no restore point could be made.".into()),
                            original: None,
                        };
                        log(log_path, batch_id, &opts, &o);
                        outcomes.push(o);
                    }
                    return ExecReport { restore_point: restore, outcomes, stopped: true, restart_needed: false };
                }
            }
            continue;
        }

        let original = match read_original(&a.kind, &opts) {
            Ok(o) => o,
            Err(e) => {
                let o = Outcome {
                    action: a.clone(),
                    status: if is_access_denied(&e) { OutcomeStatus::Protected } else { OutcomeStatus::Failed },
                    message: Some(format!("Could not read its current state: {}", e)),
                    original: None,
                };
                log(log_path, batch_id, &opts, &o);
                outcomes.push(o);
                continue;
            }
        };
        // Already in the target state: nothing to do, nothing to undo.
        let already = match (&a.kind, &original) {
            (ActionKind::DisableTask { .. }, Some(Original::Task { enabled: false })) => true,
            (ActionKind::ConfigureService { start_type, stop, .. }, Some(Original::Service { start_type: cur, running, .. })) => {
                startup_type_arg(cur) == startup_type_arg(start_type) && (!*stop || !*running)
            }
            _ => false,
        };
        let o = if already {
            Outcome {
                action: a.clone(),
                status: OutcomeStatus::Skipped,
                message: Some("Already done.".into()),
                original: None,
            }
        } else if opts.dry_run {
            Outcome {
                action: a.clone(),
                status: OutcomeStatus::WouldDo,
                message: if a.needs_admin && !opts.elevated {
                    Some("Needs administrator permission. Windows will ask when you run it for real.".into())
                } else {
                    None
                },
                original,
            }
        } else {
            match apply(&a.kind, &opts) {
                Ok(status) => Outcome {
                    action: a.clone(),
                    status,
                    message: (status == OutcomeStatus::NotConfirmed).then(|| {
                        if matches!(a.kind, ActionKind::ApplyTweak { .. }) {
                            "The registry file was imported, but not every value took effect. A policy may override it.".to_string()
                        } else {
                            "The uninstaller finished, but the app is still listed. It may need a restart, or the uninstall was cancelled.".to_string()
                        }
                    }),
                    original,
                },
                Err(e) => Outcome {
                    action: a.clone(),
                    status: if is_access_denied(&e) { OutcomeStatus::Protected } else { OutcomeStatus::Failed },
                    message: Some(if is_access_denied(&e) {
                        "Windows protects it and refused the change.".into()
                    } else {
                        e
                    }),
                    original: None,
                },
            }
        };
        log(log_path, batch_id, &opts, &o);
        outcomes.push(o);
    }
    progress(total, total, "Finished");
    let restart_needed = outcomes.iter().any(|o| {
        o.status == OutcomeStatus::Done
            && matches!(&o.action.kind, ActionKind::ApplyTweak { feature_id, .. }
                if opts.w11d.and_then(|w| w.features.iter().find(|f| &f.id == feature_id)).map(|f| f.requires_reboot).unwrap_or(false))
    });
    ExecReport { restore_point: restore, outcomes, stopped: false, restart_needed }
}
