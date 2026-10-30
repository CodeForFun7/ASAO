//! Executables referenced by startup entries, services and scheduled tasks.
//! A folder holding one of these is still in use and is never flagged.

use serde_json::Value;

use super::normalize::exe_path_from_command;

const SCRIPT: &str = r#"
$out = New-Object System.Collections.Generic.List[string]
try { Get-CimInstance Win32_Service -ErrorAction Stop | ForEach-Object { if ($_.PathName) { $out.Add([string]$_.PathName) } } } catch {}
try { Get-ScheduledTask -ErrorAction Stop | ForEach-Object { foreach ($a in $_.Actions) { if ($a.Execute) { $out.Add([string]$a.Execute) } } } } catch {}
$keys = @(
  'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run',
  'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce',
  'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run',
  'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce',
  'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run',
  'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\RunOnce'
)
foreach ($k in $keys) {
  $p = Get-ItemProperty -Path $k -ErrorAction SilentlyContinue
  if ($p) { foreach ($prop in $p.PSObject.Properties) { if ($prop.Name -notmatch '^PS') { $out.Add([string]$prop.Value) } } }
}
try {
  $shell = New-Object -ComObject WScript.Shell
  foreach ($d in @([Environment]::GetFolderPath('Startup'), [Environment]::GetFolderPath('CommonStartup'))) {
    if ($d -and (Test-Path $d)) {
      Get-ChildItem -Path $d -Filter *.lnk -ErrorAction SilentlyContinue | ForEach-Object {
        try { $t = $shell.CreateShortcut($_.FullName).TargetPath; if ($t) { $out.Add([string]$t) } } catch {}
      }
    }
  }
} catch {}
ConvertTo-Json -InputObject @($out) -Compress
"#;

/// Path keys of every referenced executable, or `None` when the lookup failed.
pub fn collect_referenced_executables() -> Option<Vec<String>> {
    let json = crate::startup::scanner::run_ps(SCRIPT.trim())?;
    let parsed: Value = serde_json::from_str(&json).ok()?;
    let items = match parsed {
        Value::Array(a) => a,
        Value::String(s) => vec![Value::String(s)],
        _ => return None,
    };
    let mut out: Vec<String> = items
        .iter()
        .filter_map(|v| v.as_str())
        .filter_map(exe_path_from_command)
        .collect();
    out.sort();
    out.dedup();
    Some(out)
}
