//! Tauri commands for the startup scanner subsystem.

use tauri::State;

use crate::analyzer::recommendation_engine::RecommendationEngine;
use crate::analyzer::ProcessInfo;
use crate::commands::AppMonitoringState;
use crate::db::ScanKind;
use crate::residual::now_ms;
use crate::startup::scanner::{
    BootPerformanceSummary, StartupItem, StartupScanner, WprController, WprStatus,
};

// ---------------------------------------------------------------------------
// Query commands
// ---------------------------------------------------------------------------

/// Collect all startup items across every supported source and correlate with live processes.
#[tauri::command]
pub fn get_startup_items(
    state: State<'_, AppMonitoringState>,
) -> Result<Vec<StartupItem>, String> {
    let started_at = now_ms();
    let timer = std::time::Instant::now();
    let items = scan_startup_items(&state)?;
    state.cache_run(
        ScanKind::Startup,
        "",
        started_at,
        timer.elapsed().as_millis() as u64,
        items.len() as u64,
        &items,
    );
    Ok(items)
}

fn scan_startup_items(state: &AppMonitoringState) -> Result<Vec<StartupItem>, String> {
    let running_procs: Vec<ProcessInfo> = if let Ok(mut engine) = state.engine.lock() {
        match engine.get_latest_or_collect() {
            Ok(snapshot) => snapshot.processes,
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let items = StartupScanner::scan(&running_procs);
    let recs = RecommendationEngine::evaluate_startup_recommendations(&items);
    if let Ok(mut guard) = state.cached_startup_recs.lock() {
        *guard = recs;
    }

    Ok(items)
}

/// Retrieve boot performance analytics and timeline phases.
#[tauri::command]
pub fn get_boot_performance_summary(
    state: State<'_, AppMonitoringState>,
) -> Result<BootPerformanceSummary, String> {
    let items = scan_startup_items(&state)?;
    Ok(WprController::compute_boot_summary(&items))
}

/// Query current Windows Performance Recorder (WPR) status.
#[tauri::command]
pub fn wpr_get_status() -> Result<WprStatus, String> {
    Ok(WprController::get_status())
}

/// Start WPR Boot Trace via Autologger.
#[tauri::command]
pub fn wpr_start_boot_trace() -> Result<String, String> {
    WprController::start_boot_trace()
}

/// Cancel configured WPR Boot Trace.
#[tauri::command]
pub fn wpr_cancel_boot_trace() -> Result<String, String> {
    WprController::cancel_boot_trace()
}

// ---------------------------------------------------------------------------
// Mutation commands (Disable / Enable)
// ---------------------------------------------------------------------------

/// Disable a startup entry.
#[tauri::command]
pub fn disable_startup_item(
    item_id: String,
    source: String,
    name: Option<String>,
    registry_key: Option<String>,
    service_name: Option<String>,
    task_path: Option<String>,
) -> Result<(), String> {
    let item_name = name.unwrap_or_else(|| clean_item_name(&item_id));
    match source.as_str() {
        "registry-run-user" | "registry-run-machine" | "registry-run-once-user" | "registry-run-once-machine" | "policy-run" => {
            disable_registry_item(&item_name, registry_key)
        }
        "startup-folder-user" | "startup-folder-machine" => {
            disable_startup_folder_item(&item_name)
        }
        "windows-service" => {
            if let Some(svc) = service_name {
                disable_service(&svc)
            } else {
                Err("No service name provided".to_string())
            }
        }
        "scheduled-task" => {
            if let Some(path) = task_path {
                disable_task(&path)
            } else {
                disable_task(&item_name)
            }
        }
        "winlogon-shell" | "winlogon-userinit" | "winlogon-notify" | "boot-execute" => {
            Err("CRITICAL SYSTEM PROTECTION: Disabling core Windows boot components is blocked to prevent system unbootability.".to_string())
        }
        other => Err(format!("Disable not supported for source: {}", other)),
    }
}

/// Re-enable a previously disabled startup entry.
#[tauri::command]
pub fn enable_startup_item(
    item_id: String,
    source: String,
    name: Option<String>,
    registry_key: Option<String>,
    service_name: Option<String>,
    task_path: Option<String>,
    command_line: Option<String>,
) -> Result<(), String> {
    let item_name = name.unwrap_or_else(|| clean_item_name(&item_id));
    match source.as_str() {
        "registry-run-user" | "registry-run-machine" | "registry-run-once-user" | "registry-run-once-machine" | "policy-run" => {
            enable_registry_item(&item_name, registry_key, command_line)
        }
        "startup-folder-user" | "startup-folder-machine" => {
            enable_startup_folder_item(&item_name)
        }
        "windows-service" => {
            if let Some(svc) = service_name {
                enable_service(&svc)
            } else {
                Err("No service name provided".to_string())
            }
        }
        "scheduled-task" => {
            if let Some(path) = task_path {
                enable_task(&path)
            } else {
                enable_task(&item_name)
            }
        }
        other => Err(format!("Enable not supported for source: {}", other)),
    }
}

// ---------------------------------------------------------------------------
// Private Helpers for Safe Toggling
// ---------------------------------------------------------------------------

fn disable_registry_item(name: &str, registry_key: Option<String>) -> Result<(), String> {
    let key_path = registry_key.unwrap_or_else(|| {
        r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run".to_string()
    });
    let item_name = ps_quote(name);

    // Use official Windows Task Manager StartupApproved mechanism (non-destructive)
    let approved_key = if key_path.contains("HKLM") {
        r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
    } else {
        r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
    };

    let ps = format!(
        r#"
$k = '{approved_key}'
if (-not (Test-Path $k)) {{ New-Item -Path $k -Force | Out-Null }}
$bytes = [byte[]]@(0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
Set-ItemProperty -Path $k -Name '{item_name}' -Value $bytes -Type Binary
"#,
        approved_key = approved_key,
        item_name = item_name
    );
    if key_path.contains("HKLM") { run_powershell_elevated(&ps) } else { run_powershell(&ps) }
}

fn enable_registry_item(
    name: &str,
    registry_key: Option<String>,
    _cmd: Option<String>,
) -> Result<(), String> {
    let key_path = registry_key.unwrap_or_else(|| {
        r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run".to_string()
    });
    let item_name = ps_quote(name);

    let approved_key = if key_path.contains("HKLM") {
        r"HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
    } else {
        r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
    };

    let ps = format!(
        r#"
$k = '{approved_key}'
if (Test-Path $k) {{
    $bytes = [byte[]]@(0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
    Set-ItemProperty -Path $k -Name '{item_name}' -Value $bytes -Type Binary
}}
"#,
        approved_key = approved_key,
        item_name = item_name
    );
    if key_path.contains("HKLM") { run_powershell_elevated(&ps) } else { run_powershell(&ps) }
}

fn disable_startup_folder_item(name: &str) -> Result<(), String> {
    let item_name = ps_quote(name);
    let ps = format!(
        r#"
$k = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder'
if (-not (Test-Path $k)) {{ New-Item -Path $k -Force | Out-Null }}
$bytes = [byte[]]@(0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
Set-ItemProperty -Path $k -Name '{item_name}.lnk' -Value $bytes -Type Binary
Set-ItemProperty -Path $k -Name '{item_name}.exe' -Value $bytes -Type Binary
"#,
        item_name = item_name
    );
    run_powershell(&ps)
}

fn enable_startup_folder_item(name: &str) -> Result<(), String> {
    let item_name = ps_quote(name);
    let ps = format!(
        r#"
$k = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder'
if (Test-Path $k) {{
    $bytes = [byte[]]@(0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
    Set-ItemProperty -Path $k -Name '{item_name}.lnk' -Value $bytes -Type Binary
    Set-ItemProperty -Path $k -Name '{item_name}.exe' -Value $bytes -Type Binary
}}
"#,
        item_name = item_name
    );
    run_powershell(&ps)
}

fn disable_service(name: &str) -> Result<(), String> {
    set_service_start(name, "Disabled", 4)
}

fn enable_service(name: &str) -> Result<(), String> {
    set_service_start(name, "Automatic", 2)
}

/// Set a service start type. Some services (e.g. sandbox services) deny
/// SERVICE_CHANGE_CONFIG even to administrators, so fall back to writing the
/// `Start` value in the service registry key directly.
fn set_service_start(name: &str, start_type: &str, start_value: u32) -> Result<(), String> {
    if name.contains('\'') {
        return Err("Unsafe characters in service name".to_string());
    }
    let ps = format!(
        "try {{ Set-Service -Name '{n}' -StartupType {t} -ErrorAction Stop }} catch {{ \
$k = 'HKLM:\\SYSTEM\\CurrentControlSet\\Services\\{n}'; \
if (-not (Test-Path -LiteralPath $k)) {{ throw $_ }}; \
Set-ItemProperty -LiteralPath $k -Name Start -Value {v} -Type DWord -ErrorAction Stop }}",
        n = name,
        t = start_type,
        v = start_value
    );
    run_powershell_elevated(&ps)
}

fn disable_task(name: &str) -> Result<(), String> {
    
    let ps = task_script("Disable-ScheduledTask", name);
    run_powershell_elevated(&ps)
}

fn enable_task(name: &str) -> Result<(), String> {
    
    let ps = task_script("Enable-ScheduledTask", name);
    run_powershell_elevated(&ps)
}

fn clean_item_name(s: &str) -> String {
    // Strip prefixes like "reg-run-user-" if present
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() > 2 && (parts[0] == "reg" || parts[0] == "startup" || parts[0] == "task" || parts[0] == "svc") {
        parts[2..].join("-")
    } else {
        s.to_string()
    }
}

fn ps_quote(s: &str) -> String {
    s.replace('\'', "''")
}

fn task_script(cmd: &str, full_path: &str) -> String {
    let (path, name) = match full_path.rfind('\\') {
        Some(i) => (&full_path[..=i], &full_path[i + 1..]),
        None => ("\\", full_path),
    };
    format!(
        "{} -TaskPath '{}' -TaskName '{}' -ErrorAction Stop | Out-Null",
        cmd,
        ps_quote(path),
        ps_quote(name)
    )
}

fn is_elevated() -> bool {
    std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)",
        ])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Run a script that needs administrator rights. If the app is not elevated,
/// relaunch PowerShell through a UAC prompt.
fn run_powershell_elevated(script: &str) -> Result<(), String> {
    if is_elevated() {
        return run_powershell(script);
    }
    let dir = std::env::temp_dir();
    let stamp = now_ms();
    let script_path = dir.join(format!("asao-startup-{}.ps1", stamp));
    let err_path = dir.join(format!("asao-startup-{}.err", stamp));
    let body = format!(
        "try {{\r\n$ErrorActionPreference = 'Stop'\r\n{}\r\n}} catch {{\r\n$_.Exception.Message | Set-Content -Path '{}'\r\nexit 1\r\n}}\r\n",
        script,
        err_path.display()
    );
    std::fs::write(&script_path, body).map_err(|e| format!("Failed to write temp script: {}", e))?;

    let launcher = format!(
        "$p = Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'; exit $p.ExitCode",
        script_path.display()
    );
    let result = run_powershell(&launcher);
    let detail = std::fs::read_to_string(&err_path).ok();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&err_path);
    match (result, detail) {
        (_, Some(d)) if !d.trim().is_empty() => Err(d.trim().to_string()),
        (Err(e), _) => Err(format!("Administrator approval was denied or failed. {}", e)),
        (Ok(()), _) => Ok(()),
    }
}

fn run_powershell(script: &str) -> Result<(), String> {
    let script = format!("$ErrorActionPreference = 'Stop'; {}", script);
    let script = script.as_str();
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            script,
        ])
        .output()
        .map_err(|e| format!("Failed to launch PowerShell: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("PowerShell error: {}", stderr.trim()))
    }
}
