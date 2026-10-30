//! Carrying out a removal. Every path starts from the user's click.

use std::path::Path;

use crate::residual::cleaner::{any_file_locked, delete_folder, processes_using};
use crate::residual::normalize::{expand_env, is_same_or_inside, path_key};
use crate::residual::roots::is_reparse_point;

#[derive(Debug)]
pub enum LaunchError {
    /// The user said no to the administrator prompt.
    Declined,
    Failed(String),
}

/// Splits an uninstall command into the program and its arguments.
/// `MsiExec.exe /I{code}` is turned into an uninstall (`/X{code}`).
pub fn split_command(cmd: &str) -> Option<(String, String)> {
    let s = cmd.trim();
    if s.is_empty() {
        return None;
    }
    let (file, rest) = if let Some(inner) = s.strip_prefix('"') {
        let end = inner.find('"')?;
        (inner[..end].to_string(), inner[end + 1..].trim().to_string())
    } else {
        let lower = s.to_ascii_lowercase();
        match lower.find(".exe") {
            Some(i) => (s[..i + 4].to_string(), s[i + 4..].trim().to_string()),
            None => match s.split_once(' ') {
                Some((f, r)) => (f.to_string(), r.trim().to_string()),
                None => (s.to_string(), String::new()),
            },
        }
    };
    let file = expand_env(&file);
    let stem = Path::new(&file)
        .file_stem()
        .map(|x| x.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let rest = if stem == "msiexec" {
        rest.split_whitespace()
            .map(|t| {
                if t.len() >= 2 && (t.starts_with("/I") || t.starts_with("/i")) {
                    format!("/X{}", &t[2..])
                } else {
                    t.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        rest
    };
    Some((file, rest))
}

/// Opens a program, document or URL through the Windows shell, like double-clicking it.
/// Uninstallers that need administrator rights show the Windows prompt themselves.
#[cfg(target_os = "windows")]
pub fn shell_open(file: &str, params: Option<&str>) -> Result<(), LaunchError> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let verb = wide("open");
    let f = wide(file);
    let p = params.filter(|p| !p.trim().is_empty()).map(wide);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(f.as_ptr()),
        lpParameters: p.as_ref().map(|v| PCWSTR(v.as_ptr())).unwrap_or(PCWSTR::null()),
        nShow: 1, // SW_SHOWNORMAL
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info).map_err(|e| {
            if (e.code().0 as u32) == 0x800704C7 {
                LaunchError::Declined
            } else {
                LaunchError::Failed(e.message().to_string())
            }
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn shell_open(_file: &str, _params: Option<&str>) -> Result<(), LaunchError> {
    Err(LaunchError::Failed("Not supported".into()))
}

/// Removes a Store app for the current user, the same as "Uninstall" in the Start menu.
pub fn remove_store_package(package_full_name: &str) -> Result<(), String> {
    if package_full_name.is_empty()
        || !package_full_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '~'))
    {
        return Err("Unexpected Store package name".into());
    }
    let script = format!("Remove-AppxPackage -Package '{}' -ErrorAction Stop", package_full_name);
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script])
        .output()
        .map_err(|e| format!("Could not start PowerShell: {}", e))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "Windows could not remove the Store app".into() } else { err })
    }
}

/// What must hold before a program folder may be removed.
pub struct FolderChecks<'a> {
    /// Path keys of programs running now.
    pub running: &'a [String],
    /// Path keys of programs started by services, scheduled tasks or startup entries.
    pub referenced: &'a [String],
    /// Folders that can never be removed as an app folder (roots, the profile, Downloads...).
    pub broad: &'a [String],
    pub windir: Option<&'a str>,
    /// ASAO's own folders.
    pub protected: &'a [String],
}

/// Refuses anything that is not plainly one app's own, idle folder.
pub fn check_folder_removable(dir: &str, checks: &FolderChecks) -> Result<(), String> {
    let key = path_key(dir);
    let meta = std::fs::symlink_metadata(dir).map_err(|_| "The folder no longer exists".to_string())?;
    if !meta.is_dir() || is_reparse_point(&meta) {
        return Err("Not a regular folder".into());
    }
    if key.len() <= 3
        || checks.broad.iter().any(|b| is_same_or_inside(b, &key))
        || checks.windir.map(|w| is_same_or_inside(&key, w)).unwrap_or(false)
    {
        return Err("This is a shared folder, not one app's folder".into());
    }
    if checks
        .protected
        .iter()
        .any(|p| is_same_or_inside(&key, p) || is_same_or_inside(p, &key))
    {
        return Err("This folder belongs to ASAO".into());
    }
    if checks.running.iter().any(|r| is_same_or_inside(r, &key)) {
        return Err("A program in this folder is running. Close it first.".into());
    }
    if checks.referenced.iter().any(|r| is_same_or_inside(r, &key)) {
        return Err("A service, scheduled task or startup entry still starts a program in this folder".into());
    }
    match processes_using(Path::new(dir)) {
        Ok(names) if !names.is_empty() => return Err(format!("In use by {}", names.join(", "))),
        Ok(_) => {}
        Err(_) => {
            if any_file_locked(Path::new(dir)) {
                return Err("A file inside is open in another program".into());
            }
        }
    }
    Ok(())
}

/// Moves the folder to the Recycle Bin, after `check_folder_removable` passed.
pub fn recycle_folder(dir: &str) -> Result<(), String> {
    delete_folder(Path::new(dir), false).map_err(|e| {
        if e.contains("0x5") || e.to_lowercase().contains("denied") {
            "Windows needs administrator permission to remove this folder. Open it in Explorer to remove it yourself.".to_string()
        } else {
            e
        }
    })
}

const SHORTCUT_SCRIPT: &str = r#"
param([string]$dir)
$shell = New-Object -ComObject WScript.Shell
$out = @()
$check = { param($f) try { $t = $shell.CreateShortcut($f.FullName).TargetPath; if ($t -and $t.ToLower().StartsWith($dir.ToLower() + '\')) { $f.FullName } } catch {} }
# The Start menu is small: search all of it. The desktop can hold whole project trees: top level only.
$programs = [Environment]::GetFolderPath('Programs')
if ($programs -and (Test-Path $programs)) {
  Get-ChildItem -Path $programs -Filter *.lnk -Recurse -ErrorAction SilentlyContinue | ForEach-Object { $out += & $check $_ }
}
$desktop = [Environment]::GetFolderPath('Desktop')
if ($desktop -and (Test-Path $desktop)) {
  Get-ChildItem -Path $desktop -Filter *.lnk -File -ErrorAction SilentlyContinue | ForEach-Object { $out += & $check $_ }
}
$out = @($out | Where-Object { $_ })
ConvertTo-Json -InputObject @($out) -Compress
"#;

/// Moves the user's Start menu and desktop shortcuts that point into `dir` to the Recycle Bin.
/// Returns how many were removed.
pub fn remove_shortcuts_into(dir: &str) -> usize {
    let dir_arg = dir.replace('\'', "''");
    let script = format!("& {{ {} }} -dir '{}'", SHORTCUT_SCRIPT, dir_arg);
    let Some(json) = crate::startup::scanner::run_ps(&script) else {
        return 0;
    };
    let list: Vec<String> = match serde_json::from_str::<serde_json::Value>(&json) {
        Ok(serde_json::Value::Array(a)) => a.into_iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect(),
        Ok(serde_json::Value::String(s)) => vec![s],
        _ => Vec::new(),
    };
    list.iter()
        .filter(|p| p.to_lowercase().ends_with(".lnk"))
        .filter(|p| delete_folder(Path::new(p), false).is_ok())
        .count()
}

/// The Steam app id of a game folder, from `steamapps\appmanifest_<id>.acf`.
pub fn steam_app_id_for(game_dir: &str) -> Option<String> {
    let game = Path::new(game_dir);
    let folder_name = game.file_name()?.to_string_lossy().to_lowercase();
    let steamapps = game.parent()?.parent()?; // ...\steamapps\common\<Game>
    for entry in std::fs::read_dir(steamapps).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        if let Some(id) = parse_acf(&text, &folder_name) {
            return Some(id);
        }
    }
    None
}

/// Reads `"appid" "440"` from an app manifest whose `"installdir"` is `folder_lower`.
pub fn parse_acf(text: &str, folder_lower: &str) -> Option<String> {
    let value = |key: &str| -> Option<String> {
        text.lines().find_map(|l| {
            let parts: Vec<&str> = l.split('"').collect();
            if parts.len() >= 4 && parts[1].eq_ignore_ascii_case(key) {
                Some(parts[3].to_string())
            } else {
                None
            }
        })
    };
    let install = value("installdir")?;
    if install.to_lowercase() != folder_lower {
        return None;
    }
    value("appid").filter(|id| id.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_uninstall_commands() {
        assert_eq!(
            split_command(r#""C:\Program Files\App\unins000.exe" /LOG"#),
            Some((r"C:\Program Files\App\unins000.exe".into(), "/LOG".into()))
        );
        assert_eq!(
            split_command(r"C:\Program Files\App\uninstall.exe --mode=gui"),
            Some((r"C:\Program Files\App\uninstall.exe".into(), "--mode=gui".into()))
        );
        assert_eq!(
            split_command("MsiExec.exe /I{11111111-2222-3333-4444-555555555555}"),
            Some(("MsiExec.exe".into(), "/X{11111111-2222-3333-4444-555555555555}".into()))
        );
        assert_eq!(
            split_command(r#""C:\Steam\steam.exe" steam://uninstall/440"#),
            Some((r"C:\Steam\steam.exe".into(), "steam://uninstall/440".into()))
        );
        assert_eq!(split_command("   "), None);
    }

    #[test]
    fn steam_manifest() {
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"440\"\n\t\"name\"\t\t\"Team Fortress 2\"\n\t\"installdir\"\t\t\"Team Fortress 2\"\n}";
        assert_eq!(parse_acf(acf, "team fortress 2").as_deref(), Some("440"));
        assert_eq!(parse_acf(acf, "other game"), None);
    }

    /// A fresh, empty folder under the system temp directory, unique per call.
    fn temp_dir(label: &str) -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "asao-apps-test-{}-{}-{}-{}",
            label,
            std::process::id(),
            crate::residual::now_ms(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn folder_checks_refuse_shared_running_and_protected() {
        let dir = temp_dir("folder");
        let app = dir.join("PortaTool");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("tool.exe"), b"x").unwrap();
        let app_s = app.to_string_lossy().to_string();
        let app_k = path_key(&app_s);
        let dir_k = path_key(&dir.to_string_lossy());

        let ok = FolderChecks { running: &[], referenced: &[], broad: &[], windir: None, protected: &[] };
        assert!(check_folder_removable(&app_s, &ok).is_ok());

        let running = vec![format!("{}\\tool.exe", app_k)];
        let busy = FolderChecks { running: &running, ..ok_checks() };
        assert!(check_folder_removable(&app_s, &busy).unwrap_err().contains("running"));

        let broad = vec![app_k.clone()];
        let shared = FolderChecks { broad: &broad, ..ok_checks() };
        assert!(check_folder_removable(&app_s, &shared).is_err());

        let protected = vec![dir_k.clone()];
        let ours = FolderChecks { protected: &protected, ..ok_checks() };
        assert!(check_folder_removable(&app_s, &ours).is_err());

        assert!(check_folder_removable(&format!("{}\\Missing", dir_k), &ok).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn ok_checks() -> FolderChecks<'static> {
        FolderChecks { running: &[], referenced: &[], broad: &[], windir: None, protected: &[] }
    }
}
