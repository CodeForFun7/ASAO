//! Evidence of apps uninstalled before ASAO started keeping its app history.
//!
//! Two sources, both readable without administrator rights:
//! - **Windows Installer log**: the Application event log records every MSI removal
//!   (event 1034) with the product name, manufacturer and date. Old events roll off.
//! - **UserAssist**: Windows keeps a per-user record of programs launched from Explorer and
//!   the Start menu, with a last-run time.
//!
//! Both also record app *updates* (an updater removes the old version; a self-updating app
//! runs from a versioned folder that later disappears). So a UserAssist program only counts
//! as uninstalled when its whole app folder is gone or holds no programs any more, and any
//! evidence matching something installed now is dropped later (see `scoring`).
//!
//! The parsers are pure functions so they can be tested without touching the machine.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::normalize::{is_same_or_inside, normalize_name, path_key};
use super::roots::is_reparse_point;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceSource {
    MsiLog,
    UserAssist,
}

#[derive(Debug, Clone)]
pub struct PastUninstall {
    pub source: EvidenceSource,
    pub display_name: String,
    pub publisher: Option<String>,
    /// Other names the app may have left folders under (exe name, for example).
    pub alt_names: Vec<String>,
    /// Folders the app used to live in (path keys).
    pub install_dirs: Vec<String>,
    pub removed_on_ms: Option<u64>,
    pub last_run_ms: Option<u64>,
}

/// Folder names that say nothing about which app it is.
const GENERIC_DIRS: &[&str] = &[
    "bin", "app", "apps", "application", "applications", "program", "programs", "current", "latest",
    "release", "x64", "x86", "win64", "win32", "amd64", "lib", "runtime", "versions", "client",
    "update", "updates", "binaries", "redist", "_redist", "games", "tools", "portable", "portableapps",
];

/// Exe names that are installers, updaters or helpers rather than the app itself.
const INSTALLER_WORDS: &[&str] = &[
    "setup", "install", "uninst", "unins0", "update", "squirrel", "maintenancetool", "cleanup",
    "crashpad", "crashreporter", "crashhandler", "helper", "redist",
];

/// Places whose programs are installers, Store app versions, shared components or temp files.
const IGNORED_LOCATIONS: &[&str] = &[
    "\\downloads\\", "\\desktop\\", "\\documents\\", "\\temp\\", "\\tmp\\", "\\$recycle.bin\\",
    "\\windowsapps\\", "\\common files\\", "\\package cache\\", "\\squirreltemp\\",
];

fn is_version_like(name: &str) -> bool {
    let lower = name.to_lowercase();
    let body = lower
        .strip_prefix("app-")
        .or_else(|| lower.strip_prefix("version-"))
        .unwrap_or(&lower);
    if lower.starts_with("version-") {
        return true;
    }
    let body = body.strip_prefix('v').unwrap_or(body);
    !body.is_empty()
        && body.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        && body.chars().any(|c| c.is_ascii_digit())
}

fn is_generic_dir(name: &str) -> bool {
    let lower = name.to_lowercase();
    GENERIC_DIRS.contains(&lower.as_str()) || is_version_like(&lower)
}

pub fn rot13(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a' + 13) % 26) + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A' + 13) % 26) + b'A') as char,
            _ => c,
        })
        .collect()
}

/// Windows FILETIME (100 ns since 1601) to Unix milliseconds.
fn filetime_to_ms(ft: u64) -> Option<u64> {
    const EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;
    if ft <= EPOCH_DIFF_100NS {
        return None;
    }
    Some((ft - EPOCH_DIFF_100NS) / 10_000)
}

/// Last-run time from a UserAssist value (the 72-byte Windows 7+ layout).
pub fn userassist_last_run_ms(data: &[u8]) -> Option<u64> {
    if data.len() < 68 {
        return None;
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&data[60..68]);
    filetime_to_ms(u64::from_le_bytes(b))
}

/// Parses `{6D809377-6AF0-444B-8957-A3773F02200E}` into its 128-bit value.
pub fn parse_guid(text: &str) -> Option<u128> {
    let hex: String = text
        .trim_matches(|c| c == '{' || c == '}')
        .chars()
        .filter(|c| *c != '-')
        .collect();
    if hex.len() != 32 {
        return None;
    }
    u128::from_str_radix(&hex, 16).ok()
}

/// What the UserAssist parser needs from the machine. Real checks in production, fakes in tests.
pub struct DiskView<'a> {
    /// Maps a known-folder GUID to its path.
    pub resolve_folder: &'a dyn Fn(u128) -> Option<String>,
    pub exists: &'a dyn Fn(&str) -> bool,
    /// Does this folder still hold any `.exe`?
    pub has_exe: &'a dyn Fn(&str) -> bool,
    /// Path keys of the folders apps are installed under (Program Files, AppData, ...).
    pub roots: &'a [String],
    /// The user's profile folder (path key). Programs in it outside AppData are not apps.
    pub profile: Option<&'a str>,
}

/// Full program path for a decoded UserAssist name. "{GUID}\Vendor\App\app.exe" uses a
/// known folder; a full path is returned as is. Non-program entries give `None`.
pub fn resolve_run_target(decoded_name: &str, resolve_folder: &dyn Fn(u128) -> Option<String>) -> Option<String> {
    if !decoded_name.to_lowercase().ends_with(".exe") {
        return None;
    }
    let full = if decoded_name.starts_with('{') {
        let end = decoded_name.find('}')?;
        let base = resolve_folder(parse_guid(&decoded_name[..=end])?)?;
        format!("{}{}", base.trim_end_matches('\\'), &decoded_name[end + 1..])
    } else {
        decoded_name.to_string()
    };
    let key = path_key(&full);
    if key.len() < 4 || key.as_bytes().get(1) != Some(&b':') {
        return None;
    }
    Some(full)
}

/// The app a program belongs to, judged from its path alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppFolder {
    /// The first meaningful folder under the root: "VideoLAN", "Docker", "Call of Duty".
    pub name: String,
    /// That folder, as written on disk.
    pub dir: String,
    /// The next meaningful folder, if any ("VLC").
    pub second: Option<String>,
    /// The program's name without ".exe".
    pub exe_stem: String,
}

/// Finds the app folder of a program: the first meaningful folder below the deepest root
/// (Program Files, AppData, Downloads, the drive, ...), skipping version and "bin"-style folders.
pub fn app_folder_for(full: &str, roots: &[String]) -> Option<AppFolder> {
    let key = path_key(full);
    let root = roots
        .iter()
        .filter(|r| is_same_or_inside(&key, r) && key.len() > r.len())
        .max_by_key(|r| r.len())
        .cloned()
        .unwrap_or_else(|| key.get(..2).unwrap_or("").to_string());
    let segments: Vec<&str> = full.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    let root_depth = root.split('\\').filter(|s| !s.is_empty()).count();
    if segments.len() <= root_depth + 1 {
        return None; // a program directly in a root says nothing about which app it is
    }
    let below = &segments[root_depth..segments.len() - 1];
    let meaningful: Vec<(usize, &str)> = below
        .iter()
        .enumerate()
        .filter(|(_, d)| !is_generic_dir(d))
        .map(|(i, d)| (i, *d))
        .take(2)
        .collect();
    let &(app_idx, first) = meaningful.first()?;
    let exe = segments[segments.len() - 1];
    Some(AppFolder {
        name: first.to_string(),
        dir: segments[..root_depth + app_idx + 1].join("\\"),
        second: meaningful.get(1).map(|(_, s)| s.to_string()),
        exe_stem: Path::new(exe)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
    })
}

/// True for exe names that are installers, updaters or helpers rather than the app itself.
pub fn is_installer_like(exe_stem: &str) -> bool {
    let lower = exe_stem.to_lowercase();
    INSTALLER_WORDS.iter().any(|w| lower.contains(w))
}

/// True for places whose programs are installers, Store app versions, shared components or temp files.
pub fn is_ignored_location(key: &str) -> bool {
    IGNORED_LOCATIONS.iter().any(|loc| key.contains(loc))
}

/// Turns one decoded UserAssist entry into evidence, when its app is gone.
pub fn userassist_entry(decoded_name: &str, last_run_ms: Option<u64>, disk: &DiskView) -> Option<PastUninstall> {
    let full = resolve_run_target(decoded_name, disk.resolve_folder)?;
    let key = path_key(&full);
    if let Ok(windir) = std::env::var("WINDIR") {
        if is_same_or_inside(&key, &path_key(&windir)) {
            return None;
        }
    }
    if is_ignored_location(&key) {
        return None;
    }
    if let Some(profile) = disk.profile {
        if is_same_or_inside(&key, profile) && !key.contains("\\appdata\\") {
            return None;
        }
    }
    if (disk.exists)(&full) {
        return None;
    }

    let app = app_folder_for(&full, disk.roots)?;
    // An updated app still has a program in its folder. Only a folder that is gone, or
    // holds no programs, says the app was uninstalled.
    if (disk.exists)(&app.dir) && (disk.has_exe)(&app.dir) {
        return None;
    }

    let display = app.name.clone();
    let mut alt_names: Vec<String> = Vec::new();
    let mut add_alt = |name: &str| {
        let norm = normalize_name(name);
        if !norm.is_empty()
            && norm != normalize_name(&display)
            && !alt_names.iter().any(|a| normalize_name(a) == norm)
        {
            alt_names.push(name.to_string());
        }
    };
    if let Some(second) = &app.second {
        add_alt(second);
    }
    if !is_installer_like(&app.exe_stem) {
        add_alt(&app.exe_stem);
    }

    Some(PastUninstall {
        source: EvidenceSource::UserAssist,
        display_name: display,
        publisher: None,
        alt_names,
        install_dirs: vec![path_key(&app.dir)],
        removed_on_ms: None,
        last_run_ms,
    })
}

/// One UserAssist launch record: the program path (or a Store app id) and its last run.
#[derive(Debug, Clone)]
pub struct RunRecord {
    /// Full program path when it is a program, otherwise the raw id ("Family_x!App").
    pub target: String,
    pub is_program: bool,
    pub last_run_ms: Option<u64>,
}

/// Every UserAssist launch record for the current user.
pub fn userassist_records() -> Vec<RunRecord> {
    read_userassist()
        .into_iter()
        .map(|(name, data)| {
            let last = userassist_last_run_ms(&data);
            match resolve_run_target(&name, &resolve_known_folder) {
                Some(full) => RunRecord { target: full, is_program: true, last_run_ms: last },
                None => RunRecord { target: name, is_program: false, last_run_ms: last },
            }
        })
        .collect()
}

/// Parses `Get-WinEvent` output for MSI removal events (id 1034), as a JSON array of
/// `{ "t": <unix ms>, "p": [name, version, language, status, manufacturer] }`.
/// Microsoft components are skipped: their folders are never flagged anyway, and most of
/// these events are Visual Studio or SDK component upgrades.
pub fn parse_msi_events(json: &str) -> Vec<PastUninstall> {
    let Ok(parsed) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let items = match parsed {
        Value::Array(a) => a,
        single @ Value::Object(_) => vec![single],
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for item in items {
        let props: Vec<String> = item["p"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())).collect())
            .unwrap_or_default();
        let name = props.first().cloned().unwrap_or_default();
        let status = props.get(3).cloned().unwrap_or_default();
        if name.trim().is_empty() || status.trim() != "0" {
            continue; // failed removals do not count
        }
        let publisher = props.get(4).cloned().filter(|p| !p.trim().is_empty() && p != "(NULL)");
        if publisher.as_deref().map(|p| p.to_lowercase().contains("microsoft")).unwrap_or(false) {
            continue;
        }
        out.push(PastUninstall {
            source: EvidenceSource::MsiLog,
            display_name: name.trim().to_string(),
            publisher,
            alt_names: Vec::new(),
            install_dirs: Vec::new(),
            removed_on_ms: item["t"].as_u64(),
            last_run_ms: None,
        });
    }
    out
}

const MSI_SCRIPT: &str = r#"
$ev = Get-WinEvent -FilterHashtable @{LogName='Application'; ProviderName='MsiInstaller'; Id=1034} -MaxEvents 3000 -ErrorAction SilentlyContinue
$out = @()
foreach ($e in $ev) {
  $out += [pscustomobject]@{ t = [int64]([DateTimeOffset]$e.TimeCreated).ToUnixTimeMilliseconds(); p = @($e.Properties | Select-Object -First 5 | ForEach-Object { [string]$_.Value }) }
}
ConvertTo-Json -InputObject @($out) -Depth 3 -Compress
"#;

#[cfg(target_os = "windows")]
fn read_userassist() -> Vec<(String, Vec<u8>)> {
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let mut out = Vec::new();
    unsafe {
        let base = wide(r"Software\Microsoft\Windows\CurrentVersion\Explorer\UserAssist");
        let mut root = HKEY::default();
        if RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(base.as_ptr()), 0, KEY_READ, &mut root) != ERROR_SUCCESS {
            return out;
        }
        let mut i = 0u32;
        loop {
            let mut name = [0u16; 128];
            let mut len = name.len() as u32;
            let rc = RegEnumKeyExW(root, i, PWSTR(name.as_mut_ptr()), &mut len, None, PWSTR::null(), None, None);
            if rc == ERROR_NO_MORE_ITEMS || i > 64 {
                break;
            }
            i += 1;
            if rc != ERROR_SUCCESS {
                continue;
            }
            let sub = format!("{}\\Count", String::from_utf16_lossy(&name[..len as usize]));
            let subw = wide(&sub);
            let mut count = HKEY::default();
            if RegOpenKeyExW(root, PCWSTR(subw.as_ptr()), 0, KEY_READ, &mut count) != ERROR_SUCCESS {
                continue;
            }
            let mut j = 0u32;
            loop {
                let mut vname = vec![0u16; 2048];
                let mut vlen = vname.len() as u32;
                let mut data = vec![0u8; 4096];
                let mut dlen = data.len() as u32;
                let rc = RegEnumValueW(
                    count,
                    j,
                    PWSTR(vname.as_mut_ptr()),
                    &mut vlen,
                    None,
                    None,
                    Some(data.as_mut_ptr()),
                    Some(&mut dlen),
                );
                if rc == ERROR_NO_MORE_ITEMS || j > 20_000 {
                    break;
                }
                j += 1;
                if rc != ERROR_SUCCESS {
                    continue;
                }
                data.truncate(dlen as usize);
                out.push((rot13(&String::from_utf16_lossy(&vname[..vlen as usize])), data));
            }
            let _ = RegCloseKey(count);
        }
        let _ = RegCloseKey(root);
    }
    out
}

#[cfg(not(target_os = "windows"))]
fn read_userassist() -> Vec<(String, Vec<u8>)> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn resolve_known_folder(id: u128) -> Option<String> {
    use windows::core::GUID;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    let guid = GUID::from_u128(id);
    unsafe {
        let p = SHGetKnownFolderPath(&guid, KF_FLAG_DEFAULT, HANDLE::default()).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
        s
    }
}

#[cfg(not(target_os = "windows"))]
fn resolve_known_folder(_id: u128) -> Option<String> {
    None
}

/// Reparse-safe check for any `.exe` inside a folder (bounded walk).
fn folder_has_exe(path: &str) -> bool {
    let mut stack = vec![PathBuf::from(path)];
    let mut seen = 0u32;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            seen += 1;
            if seen > 20_000 {
                return true; // too big to be sure: treat as still installed
            }
            let p = entry.path();
            let Ok(meta) = fs::symlink_metadata(&p) else { continue };
            if is_reparse_point(&meta) {
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .map(|e| e.eq_ignore_ascii_case("exe"))
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

/// Collects evidence from this machine. Never fails: a source that cannot be read adds nothing.
pub fn collect_past_uninstalls() -> Vec<PastUninstall> {
    let mut out = Vec::new();
    if let Some(json) = crate::startup::scanner::run_ps(MSI_SCRIPT.trim()) {
        out.extend(parse_msi_events(&json));
    }

    let mut roots: Vec<String> = super::roots::known_roots()
        .iter()
        .map(|r| path_key(&r.path.to_string_lossy()))
        .collect();
    let profile = std::env::var("USERPROFILE").ok().map(|p| path_key(&p));
    if let Some(p) = &profile {
        roots.push(p.clone());
    }
    let exists = |p: &str| Path::new(p).exists();
    let disk = DiskView {
        resolve_folder: &resolve_known_folder,
        exists: &exists,
        has_exe: &folder_has_exe,
        roots: &roots,
        profile: profile.as_deref(),
    };
    for (name, data) in read_userassist() {
        if let Some(e) = userassist_entry(&name, userassist_last_run_ms(&data), &disk) {
            out.push(e);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PF: u128 = 0x6D809377_6AF0_444B_8957_A3773F02200E;

    fn resolver(id: u128) -> Option<String> {
        if id == PF {
            Some(r"C:\Program Files".into())
        } else {
            None
        }
    }

    fn roots() -> Vec<String> {
        vec![
            r"c:\program files".into(),
            r"c:\users\x\appdata\local".into(),
            r"c:\users\x\appdata\roaming".into(),
            r"c:\users\x".into(),
        ]
    }

    fn view<'a>(exists: &'a dyn Fn(&str) -> bool, has_exe: &'a dyn Fn(&str) -> bool, roots: &'a [String]) -> DiskView<'a> {
        DiskView {
            resolve_folder: &resolver,
            exists,
            has_exe,
            roots,
            profile: Some(r"c:\users\x"),
        }
    }

    #[test]
    fn rot13_and_guid() {
        assert_eq!(rot13("{6Q809377}\\IvqrbYNA\\IYP\\iyp.rkr"), "{6D809377}\\VideoLAN\\VLC\\vlc.exe");
        assert_eq!(parse_guid("{6D809377-6AF0-444B-8957-A3773F02200E}"), Some(PF));
        assert_eq!(parse_guid("{bad}"), None);
    }

    #[test]
    fn last_run_from_72_byte_value() {
        let mut data = vec![0u8; 72];
        // 2026-01-01T00:00:00Z as FILETIME
        let ft: u64 = 134_116_992_000_000_000;
        data[60..68].copy_from_slice(&ft.to_le_bytes());
        assert_eq!(userassist_last_run_ms(&data), Some(1_767_225_600_000));
        assert_eq!(userassist_last_run_ms(&[0u8; 16]), None);
    }

    #[test]
    fn removed_program_becomes_evidence_named_from_top_folders() {
        let r = roots();
        let gone = |_: &str| false;
        let no_exe = |_: &str| false;
        let d = view(&gone, &no_exe, &r);

        let e = userassist_entry(r"{6D809377-6AF0-444B-8957-A3773F02200E}\VideoLAN\VLC\vlc.exe", Some(5), &d).unwrap();
        assert_eq!(e.display_name, "VideoLAN");
        assert_eq!(e.alt_names, vec!["VLC".to_string()]); // "vlc.exe" adds nothing new
        assert_eq!(e.install_dirs, vec![r"c:\program files\videolan".to_string()]);
        assert_eq!(e.last_run_ms, Some(5));

        // A container folder like "Games" is skipped when naming
        let e = userassist_entry(r"C:\Games\Call of Duty\sp22\sp22-cod.exe", None, &d).unwrap();
        assert_eq!(e.display_name, "Call of Duty");
        assert_eq!(e.alt_names, vec!["sp22".to_string(), "sp22-cod".to_string()]);

        // Deep helper folders do not name the app
        let e = userassist_entry(r"C:\Program Files\Docker\Docker\frontend\Docker Desktop.exe", None, &d).unwrap();
        assert_eq!(e.display_name, "Docker");
        assert_eq!(e.alt_names, vec!["Docker Desktop".to_string()]);

        // Version folders are skipped; installer exe names are not used as names
        let e = userassist_entry(r"C:\Users\x\AppData\Local\Fooapp\app-1.2.3\Foo.exe", None, &d).unwrap();
        assert_eq!(e.display_name, "Fooapp");
        assert_eq!(e.alt_names, vec!["Foo".to_string()]);
        let e = userassist_entry(r"C:\Users\x\AppData\Local\Barapp\Update\barapp_installer-1.2.exe", None, &d).unwrap();
        assert_eq!(e.display_name, "Barapp");
        assert!(e.alt_names.is_empty());
    }

    #[test]
    fn updated_apps_and_noise_are_ignored() {
        let r = roots();
        let gone = |_: &str| false;
        let no_exe = |_: &str| false;
        let d = view(&gone, &no_exe, &r);

        // The app folder still exists and still holds a program: it was updated, not removed
        let folder_exists = |p: &str| !p.to_lowercase().ends_with(".exe");
        let has_exe = |_: &str| true;
        let updated = view(&folder_exists, &has_exe, &r);
        assert!(userassist_entry(r"C:\Users\x\AppData\Local\Roblox\Versions\version-abc\RobloxPlayerBeta.exe", None, &updated).is_none());

        let present = |_: &str| true;
        let installed = view(&present, &has_exe, &r);
        assert!(userassist_entry(r"C:\Program Files\Tool\tool.exe", None, &installed).is_none());

        assert!(userassist_entry(r"C:\Users\x\Downloads\Thing Setup.exe", None, &d).is_none());
        assert!(userassist_entry(r"C:\Program Files\WindowsApps\Claude_1.2.3_x64__abc\app\Claude.exe", None, &d).is_none());
        assert!(userassist_entry(r"C:\Program Files\Common Files\EAInstaller\Game\Cleanup.exe", None, &d).is_none());
        assert!(userassist_entry(r"C:\Users\x\fire\fire.exe", None, &d).is_none());
        assert!(userassist_entry(r"Microsoft.Windows.Explorer", None, &d).is_none());
        assert!(userassist_entry(r"C:\tool.exe", None, &d).is_none());
        assert!(userassist_entry(r"C:\Program Files\tool.exe", None, &d).is_none());
    }

    #[test]
    fn msi_events_keep_successful_non_microsoft_removals() {
        let json = r#"[
            {"t": 1000, "p": ["Contoso Studio", "2.1", "1033", "0", "Contoso Ltd"]},
            {"t": 2000, "p": ["Broken Tool", "1.0", "1033", "1603", "Fabrikam"]},
            {"t": 3000, "p": ["No Maker", "1.0", "1033", "0", "(NULL)"]},
            {"t": 4000, "p": ["vs_minshellmsires", "18.4", "1033", "0", "Microsoft Corporation"]}
        ]"#;
        let ev = parse_msi_events(json);
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].display_name, "Contoso Studio");
        assert_eq!(ev[0].publisher.as_deref(), Some("Contoso Ltd"));
        assert_eq!(ev[0].removed_on_ms, Some(1000));
        assert_eq!(ev[1].publisher, None);
    }
}
