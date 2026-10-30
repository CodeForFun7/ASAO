//! Installed-app inventory. Each source is an `InventoryProvider`, so game launchers
//! (Steam, Epic, …) can be added later without touching the rest of the pipeline.

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

pub trait InventoryProvider {
    fn name(&self) -> &'static str;
    /// `None` when the source could not be read at all (as opposed to "no apps").
    fn collect(&self) -> Option<Vec<InstalledApp>>;
}

pub struct RegistryInventory;
pub struct StoreInventory;

#[derive(Debug, Clone, Default)]
pub struct InventoryResult {
    pub apps: Vec<InstalledApp>,
    /// Source classes ("registry", "store") that failed to load this time.
    pub failed_classes: Vec<&'static str>,
}

pub fn default_providers() -> Vec<Box<dyn InventoryProvider + Send>> {
    vec![Box::new(RegistryInventory), Box::new(StoreInventory)]
}

pub fn collect_installed_apps() -> InventoryResult {
    let mut result = InventoryResult::default();
    for provider in default_providers() {
        match provider.collect() {
            Some(apps) => result.apps.extend(apps),
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

    fn collect(&self) -> Option<Vec<InstalledApp>> {
        registry::collect()
    }
}

impl InventoryProvider for StoreInventory {
    fn name(&self) -> &'static str {
        "store"
    }

    fn collect(&self) -> Option<Vec<InstalledApp>> {
        let script = "Get-AppxPackage | Select-Object Name,PackageFamilyName,InstallLocation,Publisher,IsFramework | ConvertTo-Json -Compress";
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
            out.push(InstalledApp {
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
            });
        }
        Some(out)
    }
}

#[cfg(target_os = "windows")]
mod registry {
    use super::{AppSource, InstalledApp};
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

    pub fn collect() -> Option<Vec<InstalledApp>> {
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
                out.push(InstalledApp {
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
                });
            }
        }
        if opened_any {
            Some(out)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod registry {
    pub fn collect() -> Option<Vec<super::InstalledApp>> {
        None
    }
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
    fn derives_folder_when_install_location_is_empty() {
        let a = app(None, Some(r#""C:\Program Files\Foo\unins000.exe" /SILENT"#), None);
        assert_eq!(a.owned_dirs(), vec![r"c:\program files\foo".to_string()]);
        let b = app(None, None, Some(r"C:\Users\x\AppData\Local\Bar\bar.exe,0"));
        assert_eq!(b.owned_dirs(), vec![r"c:\users\x\appdata\local\bar".to_string()]);
        let c = app(Some(r"C:\Program Files\Baz\"), None, None);
        assert_eq!(c.owned_dirs(), vec![r"c:\program files\baz".to_string()]);
    }
}
