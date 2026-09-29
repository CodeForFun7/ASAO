//! Tauri commands for the startup scanner subsystem.

use tauri::State;

use crate::analyzer::recommendation_engine::RecommendationEngine;
use crate::analyzer::ProcessInfo;
use crate::commands::AppMonitoringState;
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
    let items = get_startup_items(state)?;
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
    registry_key: Option<String>,
    service_name: Option<String>,
    task_path: Option<String>,
) -> Result<(), String> {
    let _ = item_id;
    match source.as_str() {
        "registry-run-user" | "registry-run-machine" | "registry-run-once-user" | "registry-run-once-machine" | "policy-run" => {
            disable_registry_item(&item_id, registry_key)
        }
        "startup-folder-user" | "startup-folder-machine" => {
            disable_startup_folder_item(&item_id)
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
                disable_task(&item_id)
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
    registry_key: Option<String>,
    service_name: Option<String>,
    task_path: Option<String>,
    command_line: Option<String>,
) -> Result<(), String> {
    let _ = item_id;
    match source.as_str() {
        "registry-run-user" | "registry-run-machine" | "registry-run-once-user" | "registry-run-once-machine" | "policy-run" => {
            enable_registry_item(&item_id, registry_key, command_line)
        }
        "startup-folder-user" | "startup-folder-machine" => {
            enable_startup_folder_item(&item_id)
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
                enable_task(&item_id)
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
    let item_name = clean_item_name(name);

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
    run_powershell(&ps)
}

fn enable_registry_item(
    name: &str,
    registry_key: Option<String>,
    _cmd: Option<String>,
) -> Result<(), String> {
    let key_path = registry_key.unwrap_or_else(|| {
        r"HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run".to_string()
    });
    let item_name = clean_item_name(name);

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
    run_powershell(&ps)
}

fn disable_startup_folder_item(name: &str) -> Result<(), String> {
    let item_name = clean_item_name(name);
    let ps = format!(
        r#"
$k = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder'
if (-not (Test-Path $k)) {{ New-Item -Path $k -Force | Out-Null }}
$bytes = [byte[]]@(0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
Set-ItemProperty -Path $k -Name '{item_name}.lnk' -Value $bytes -Type Binary -ErrorAction SilentlyContinue
Set-ItemProperty -Path $k -Name '{item_name}' -Value $bytes -Type Binary -ErrorAction SilentlyContinue
"#,
        item_name = item_name
    );
    run_powershell(&ps)
}

fn enable_startup_folder_item(name: &str) -> Result<(), String> {
    let item_name = clean_item_name(name);
    let ps = format!(
        r#"
$k = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder'
if (Test-Path $k) {{
    $bytes = [byte[]]@(0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
    Set-ItemProperty -Path $k -Name '{item_name}.lnk' -Value $bytes -Type Binary -ErrorAction SilentlyContinue
    Set-ItemProperty -Path $k -Name '{item_name}' -Value $bytes -Type Binary -ErrorAction SilentlyContinue
}}
"#,
        item_name = item_name
    );
    run_powershell(&ps)
}

fn disable_service(name: &str) -> Result<(), String> {
    if name.contains('\'') {
        return Err("Unsafe characters in service name".to_string());
    }
    let ps = format!("Set-Service -Name '{}' -StartupType Disabled", name);
    run_powershell(&ps)
}

fn enable_service(name: &str) -> Result<(), String> {
    if name.contains('\'') {
        return Err("Unsafe characters in service name".to_string());
    }
    let ps = format!("Set-Service -Name '{}' -StartupType Automatic", name);
    run_powershell(&ps)
}

fn disable_task(name: &str) -> Result<(), String> {
    let clean = clean_item_name(name);
    let ps = format!("Disable-ScheduledTask -TaskName '{}' -ErrorAction SilentlyContinue", clean);
    run_powershell(&ps)
}

fn enable_task(name: &str) -> Result<(), String> {
    let clean = clean_item_name(name);
    let ps = format!("Enable-ScheduledTask -TaskName '{}' -ErrorAction SilentlyContinue", clean);
    run_powershell(&ps)
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

fn run_powershell(script: &str) -> Result<(), String> {
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
