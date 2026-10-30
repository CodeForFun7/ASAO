//! Installed-app inventory. Each source is an `InventoryProvider`, so game launchers
//! (Steam, Epic, …) can be added later without touching the rest of the pipeline.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::normalize::{dir_from_command, dir_from_icon, expand_env, path_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppSource {
    RegistryMachine,
    RegistryMachineWow64,
    RegistryUser,
    Store,
}

impl AppSource {
    /// Coarse class used to guard against a whole source failing to load.
    pub fn class(&self) -> &'static str {
        match self {
            AppSource::Store => "store",
            _ => "registry",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub display_name: Option<String>,
    pub publisher: Option<String>,
    pub install_location: Option<String>,
    pub uninstall_string: Option<String>,
    pub display_icon: Option<String>,
    pub source: AppSource,
    /// No display name, or `SystemComponent = 1`. Hidden from lists but still "installed".
    pub hidden: bool,
    pub package_family_name: Option<String>,
    pub package_name: Option<String>,
}

impl InstalledApp {
    /// Folders that belong to this app: the install location, or (when that is empty)
    /// the folders of its uninstaller and icon. Returned as path keys.
    pub fn owned_dirs(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(loc) = self.install_location.as_deref() {
            let expanded = expand_env(loc.trim().trim_matches('"'));
            if expanded.len() >= 3 {
                out.push(path_key(&expanded));
            }
        }
        if let Some(cmd) = self.uninstall_string.as_deref() {
            if let Some(dir) = dir_from_command(cmd) {
                out.push(dir);
            }
        }
        if let Some(icon) = self.display_icon.as_deref() {
            if let Some(dir) = dir_from_icon(icon) {
                out.push(dir);
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Extra facts about an installed app, used by the Unused apps tab. Kept beside
/// `InstalledApp` (keyed by its id) so the ownership code stays unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDetails {
    /// From the registry `InstallDate` (yyyymmdd), as Unix milliseconds.
    pub install_date_ms: Option<u64>,
    /// From the registry `EstimatedSize` (KB), in bytes.
    pub estimated_size_bytes: Option<u64>,
    /// The uninstall key's own name. For MSI installs this is the product code `{GUID}`.
    pub registry_subkey: Option<String>,
    pub package_full_name: Option<String>,
    /// Store: "Store", "System", "Developer", ...
    pub signature_kind: Option<String>,
    pub non_removable: bool,
    pub is_framework: bool,
}

pub trait InventoryProvider {
    fn name(&self) -> &'static str;
    /// `None` when the source could not be read at all (as opposed to "no apps").
    fn collect(&self) -> Option<Vec<(InstalledApp, AppDetails)>>;
}

pub struct RegistryInventory;
pub struct StoreInventory;

#[derive(Debug, Clone, Default)]
pub struct InventoryResult {
    pub apps: Vec<InstalledApp>,
    /// Source classes ("registry", "store") that failed to load this time.
    pub failed_classes: Vec<&'static str>,
    /// Extra facts per app id.
    pub details: HashMap<String, AppDetails>,
}

pub fn default_providers() -> Vec<Box<dyn InventoryProvider + Send>> {
    vec![Box::new(RegistryInventory), Box::new(StoreInventory)]
}

pub fn collect_installed_apps() -> InventoryResult {
    let mut result = InventoryResult::default();
    for provider in default_providers() {
        match provider.collect() {
            Some(apps) => {
                for (app, details) in apps {
                    result.details.insert(app.id.clone(), details);
                    result.apps.push(app);
                }
            }
            None => {
                let class = if provider.name() == "store" { "store" } else { "registry" };
                result.failed_classes.push(class);
            }
        }
    }
    result
}

impl InventoryProvider for RegistryInventory {
    fn name(&self) -> &'static str {
        "registry"
    }

    fn collect(&self) -> Option<Vec<(InstalledApp, AppDetails)>> {
        registry::collect()
    }
}

impl InventoryProvider for StoreInventory {
    fn name(&self) -> &'static str {
        "store"
    }

    fn collect(&self) -> Option<Vec<(InstalledApp, AppDetails)>> {
        let script = "Get-AppxPackage | Select-Object Name,PackageFamilyName,PackageFullName,InstallLocation,Publisher,IsFramework,NonRemovable,@{n='SignatureKind';e={[string]$_.SignatureKind}} | ConvertTo-Json -Compress";
        let json = crate::startup::scanner::run_ps(script)?;
        let parsed: Value = serde_json::from_str(&json).ok()?;
        let items = match parsed {
            Value::Array(a) => a,
            single @ Value::Object(_) => vec![single],
            _ => Vec::new(),
        };
        let mut out = Vec::new();
        for item in items {
            let name = item["Name"].as_str().unwrap_or("").to_string();
            let family = item["PackageFamilyName"].as_str().unwrap_or("").to_string();
            if name.is_empty() || family.is_empty() {
                continue;
            }
            let publisher = item["Publisher"].as_str().map(|p| {
                // "CN=Contoso, O=Contoso Ltd, C=US" becomes "Contoso"
                p.split(',')
                    .find_map(|part| part.trim().strip_prefix("CN=").map(|s| s.to_string()))
                    .unwrap_or_else(|| p.to_string())
            });
            let is_framework = item["IsFramework"].as_bool().unwrap_or(false);
            let details = AppDetails {
                package_full_name: item["PackageFullName"].as_str().map(|s| s.to_string()),
                signature_kind: item["SignatureKind"].as_str().map(|s| s.to_string()),
                non_removable: item["NonRemovable"].as_bool().unwrap_or(false),
                is_framework,
                ..Default::default()
            };
            out.push((
                InstalledApp {
                    id: format!("store:{}", family.to_lowercase()),
                    display_name: Some(name.clone()),
                    publisher,
                    install_location: item["InstallLocation"].as_str().map(|s| s.to_string()),
                    uninstall_string: None,
                    display_icon: None,
                    source: AppSource::Store,
                    hidden: is_framework,
                    package_family_name: Some(family),
                    package_name: Some(name),
                },
                details,
            ));
        }
        Some(out)
    }
}

/// "20240131" as Unix milliseconds (UTC midnight).
pub fn parse_install_date(text: &str) -> Option<u64> {
    let t = text.trim();
    if t.len() != 8 || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let y: i64 = t[0..4].parse().ok()?;
    let m: i64 = t[4..6].parse().ok()?;
    let d: i64 = t[6..8].parse().ok()?;
    if !(1990..=2200).contains(&y) || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil date (Howard Hinnant's algorithm)
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400_000)
}

/// Quick registry-only read, used to watch for an uninstall to finish.
pub fn collect_registry_apps() -> Option<Vec<InstalledApp>> {
    registry::collect().map(|v| v.into_iter().map(|(a, _)| a).collect())
}

#[cfg(target_os = "windows")]
mod registry {
    use super::{AppDetails, AppSource, InstalledApp};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
        HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_DWORD,
        REG_EXPAND_SZ, REG_SAM_FLAGS, REG_SZ, REG_VALUE_TYPE,
    };

    const UNINSTALL: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    fn open(parent: HKEY, sub: &str, sam: REG_SAM_FLAGS) -> Option<Key> {
        let w = wide(sub);
        let mut out = HKEY::default();
        let rc = unsafe { RegOpenKeyExW(parent, PCWSTR(w.as_ptr()), 0, sam, &mut out) };
        if rc == ERROR_SUCCESS {
            Some(Key(out))
        } else {
            None
        }
    }

    fn subkeys(key: &Key) -> Vec<String> {
        let mut names = Vec::new();
        let mut index = 0u32;
        loop {
            let mut buf = [0u16; 512];
            let mut len = buf.len() as u32;
            let rc = unsafe {
                RegEnumKeyExW(
                    key.0,
                    index,
                    PWSTR(buf.as_mut_ptr()),
                    &mut len,
                    None,
                    PWSTR::null(),
                    None,
                    None,
                )
            };
            if rc == ERROR_NO_MORE_ITEMS {
                break;
            }
            if rc == ERROR_SUCCESS {
                names.push(String::from_utf16_lossy(&buf[..len as usize]));
            }
            index += 1;
            if index > 20_000 {
                break;
            }
        }
        names
    }

    fn read_string(key: &Key, name: &str) -> Option<String> {
        let w = wide(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0u32;
        let rc = unsafe {
            RegQueryValueExW(key.0, PCWSTR(w.as_ptr()), None, Some(&mut kind), None, Some(&mut size))
        };
        if (rc != ERROR_SUCCESS && rc != ERROR_MORE_DATA) || size == 0 {
            return None;
        }
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            return None;
        }
        let mut buf = vec![0u16; (size as usize / 2) + 1];
        let mut bytes = (buf.len() * 2) as u32;
        let rc = unsafe {
            RegQueryValueExW(
                key.0,
                PCWSTR(w.as_ptr()),
                None,
                Some(&mut kind),
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut bytes),
            )
        };
        if rc != ERROR_SUCCESS {
            return None;
        }
        let chars = (bytes as usize / 2).min(buf.len());
        let end = buf[..chars].iter().position(|&c| c == 0).unwrap_or(chars);
        let s = String::from_utf16_lossy(&buf[..end]).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    fn read_dword(key: &Key, name: &str) -> Option<u32> {
        let w = wide(name);
        let mut kind = REG_VALUE_TYPE::default();
        let mut data = 0u32;
        let mut size = 4u32;
        let rc = unsafe {
            RegQueryValueExW(
                key.0,
                PCWSTR(w.as_ptr()),
                None,
                Some(&mut kind),
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut size),
            )
        };
        if rc == ERROR_SUCCESS && kind == REG_DWORD {
            Some(data)
        } else {
            None
        }
    }

    pub fn collect() -> Option<Vec<(InstalledApp, AppDetails)>> {
        let sources: [(HKEY, REG_SAM_FLAGS, AppSource, &str); 3] = [
            (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_64KEY, AppSource::RegistryMachine, "hklm"),
            (HKEY_LOCAL_MACHINE, KEY_READ | KEY_WOW64_32KEY, AppSource::RegistryMachineWow64, "hklm32"),
            (HKEY_CURRENT_USER, KEY_READ, AppSource::RegistryUser, "hkcu"),
        ];
        let mut out = Vec::new();
        let mut opened_any = false;
        for (hive, sam, source, prefix) in sources {
            let Some(root) = open(hive, UNINSTALL, sam) else {
                continue;
            };
            opened_any = true;
            for sub in subkeys(&root) {
                let Some(app_key) = open(root.0, &sub, sam) else {
                    continue;
                };
                let display_name = read_string(&app_key, "DisplayName");
                let system_component = read_dword(&app_key, "SystemComponent").unwrap_or(0) == 1;
                let details = AppDetails {
                    install_date_ms: read_string(&app_key, "InstallDate")
                        .as_deref()
                        .and_then(super::parse_install_date),
                    estimated_size_bytes: read_dword(&app_key, "EstimatedSize")
                        .filter(|kb| *kb > 0)
                        .map(|kb| kb as u64 * 1024),
                    registry_subkey: Some(sub.clone()),
                    ..Default::default()
                };
                out.push((
                    InstalledApp {
                        id: format!("{}:{}", prefix, sub.to_lowercase()),
                        hidden: display_name.is_none() || system_component,
                        display_name,
                        publisher: read_string(&app_key, "Publisher"),
                        install_location: read_string(&app_key, "InstallLocation"),
                        uninstall_string: read_string(&app_key, "UninstallString"),
                        display_icon: read_string(&app_key, "DisplayIcon"),
                        source,
                        package_family_name: None,
                        package_name: None,
                    },
                    details,
                ));
            }
        }
        if opened_any {
            Some(out)
        } else {
            None
        }
    }

    /// The saved user and machine PATH. A running process keeps the PATH it started with,
    /// so a tool installed later is only found here.
    pub fn saved_path() -> Vec<String> {
        let mut out = Vec::new();
        let places = [
            (HKEY_CURRENT_USER, r"Environment"),
            (HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment"),
        ];
        for (hive, sub) in places {
            if let Some(key) = open(hive, sub, KEY_READ) {
                if let Some(v) = read_string(&key, "Path") {
                    out.push(v);
                }
            }
        }
        out
    }
}

#[cfg(not(target_os = "windows"))]
mod registry {
    pub fn collect() -> Option<Vec<(super::InstalledApp, super::AppDetails)>> {
        None
    }

    pub fn saved_path() -> Vec<String> {
        Vec::new()
    }
}

/// The user and machine PATH as saved in the registry (unexpanded `%VAR%` included).
pub fn saved_path() -> Vec<String> {
    registry::saved_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(loc: Option<&str>, uninst: Option<&str>, icon: Option<&str>) -> InstalledApp {
        InstalledApp {
            id: "t".into(),
            display_name: Some("Test".into()),
            publisher: None,
            install_location: loc.map(|s| s.to_string()),
            uninstall_string: uninst.map(|s| s.to_string()),
            display_icon: icon.map(|s| s.to_string()),
            source: AppSource::RegistryUser,
            hidden: false,
            package_family_name: None,
            package_name: None,
        }
    }

    #[test]
    fn parses_install_dates() {
        assert_eq!(parse_install_date("19900101"), Some(631_152_000_000));
        assert_eq!(parse_install_date("19700101"), None); // before any real install date
        assert_eq!(parse_install_date("20260101"), Some(1_767_225_600_000));
        assert_eq!(parse_install_date("2026-01-01"), None);
        assert_eq!(parse_install_date("20261340"), None);
    }

    #[test]
    fn derives_folder_when_install_location_is_empty() {
        let a = app(None, Some(r#""C:\Program Files\Foo\unins000.exe" /SILENT"#), None);
        assert_eq!(a.owned_dirs(), vec![r"c:\program files\foo".to_string()]);
        let b = app(None, None, Some(r"C:\Users\x\AppData\Local\Bar\bar.exe,0"));
        assert_eq!(b.owned_dirs(), vec![r"c:\users\x\appdata\local\bar".to_string()]);
        let c = app(Some(r"C:\Program Files\Baz\"), None, None);
        assert_eq!(c.owned_dirs(), vec![r"c:\program files\baz".to_string()]);
    }
}
