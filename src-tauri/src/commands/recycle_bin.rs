//! Recycle Bin: list what is in it (all drives) and permanently delete chosen items.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::residual::sizer;

/// Lists every item through the Shell, which knows each item's original name and location.
/// `$i.Path` is the item's real location inside `$Recycle.Bin`.
const LIST_SCRIPT: &str = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$out = New-Object System.Collections.Generic.List[object]
$bin = (New-Object -ComObject Shell.Application).NameSpace(10)
if ($bin) {
  foreach ($i in $bin.Items()) {
    $deleted = $null
    try { $d = $i.ExtendedProperty('System.Recycle.DateDeleted'); if ($d) { $deleted = ([DateTimeOffset][datetime]$d).ToUnixTimeMilliseconds() } } catch {}
    $size = $null
    try { $s = $i.ExtendedProperty('System.Size'); if ($s -ne $null) { $size = [uint64]$s } } catch {}
    $from = $null
    try { $from = [string]$i.ExtendedProperty('System.Recycle.DeletedFrom') } catch {}
    $out.Add([pscustomobject]@{
      name = [string]$i.Name
      path = [string]$i.Path
      originalFolder = $from
      isFolder = [bool]$i.IsFolder
      typeName = [string]$i.Type
      sizeBytes = $size
      deletedMs = $deleted
    })
  }
}
ConvertTo-Json -InputObject $out.ToArray() -Compress -Depth 3
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBinItem {
    pub name: String,
    /// Where the item sits inside `$Recycle.Bin`.
    pub path: String,
    /// The folder it was deleted from.
    pub original_folder: Option<String>,
    pub is_folder: bool,
    pub type_name: Option<String>,
    pub size_bytes: Option<u64>,
    pub deleted_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBinListing {
    pub items: Vec<RecycleBinItem>,
    /// Totals from Windows, across every drive.
    pub total_bytes: u64,
    pub total_items: u64,
}

/// Windows hides known extensions in the Shell name, so take the extension from the real file.
fn display_name(item: &RecycleBinItem) -> String {
    if item.is_folder {
        return item.name.clone();
    }
    let ext = Path::new(&item.path)
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_default();
    if ext.is_empty() || item.name.to_lowercase().ends_with(&format!(".{}", ext.to_lowercase())) {
        item.name.clone()
    } else {
        format!("{}.{}", item.name, ext)
    }
}

#[cfg(target_os = "windows")]
fn query_totals() -> Result<(u64, u64), String> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO};

    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) }
        .map_err(|e| format!("Could not read the Recycle Bin: {}", e))?;
    Ok((info.i64Size.max(0) as u64, info.i64NumItems.max(0) as u64))
}

#[cfg(not(target_os = "windows"))]
fn query_totals() -> Result<(u64, u64), String> {
    Err("Not supported".into())
}

fn list_blocking() -> Result<RecycleBinListing, String> {
    let (total_bytes, total_items) = query_totals()?;
    if total_items == 0 {
        return Ok(RecycleBinListing::default());
    }

    let json = crate::startup::scanner::run_ps(LIST_SCRIPT.trim())
        .ok_or_else(|| "Could not read the Recycle Bin".to_string())?;
    let mut items: Vec<RecycleBinItem> = match serde_json::from_str::<serde_json::Value>(&json) {
        Ok(serde_json::Value::Array(a)) => a
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
        Ok(v @ serde_json::Value::Object(_)) => serde_json::from_value(v).ok().into_iter().collect(),
        _ => return Err("Could not read the Recycle Bin".into()),
    };

    let cancel = AtomicBool::new(false);
    for item in &mut items {
        item.name = display_name(item);
        if item.original_folder.as_deref() == Some("") {
            item.original_folder = None;
        }
        // The Shell reports no size for folders, so measure them.
        if item.is_folder || item.size_bytes.is_none() {
            item.size_bytes = Some(sizer::measure(Path::new(&item.path), &cancel).size_on_disk);
        }
    }
    items.sort_by(|a, b| b.deleted_ms.cmp(&a.deleted_ms));

    Ok(RecycleBinListing {
        items,
        total_bytes,
        total_items,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBinDeleteFailure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycleBinDeleteReport {
    pub deleted: u64,
    pub freed_bytes: u64,
    pub failed: Vec<RecycleBinDeleteFailure>,
    /// What is in the bin afterwards.
    pub listing: RecycleBinListing,
}

/// Only `X:\$Recycle.Bin\<user SID>\$R...` may be deleted from here. Nothing else.
fn is_recycle_bin_item(path: &Path) -> bool {
    use std::path::Component;
    let parts: Vec<Component> = path.components().collect();
    let [Component::Prefix(_), Component::RootDir, Component::Normal(bin), Component::Normal(sid), Component::Normal(name)] =
        parts.as_slice()
    else {
        return false;
    };
    let name = name.to_string_lossy();
    bin.to_string_lossy().eq_ignore_ascii_case("$Recycle.Bin")
        && sid.to_string_lossy().to_uppercase().starts_with("S-1-")
        && name.len() > 2
        && name.starts_with("$R")
}

/// The `$I` file next to a `$R` item holds its original name and date. Without it the
/// item would disappear; without the `$R` the bin would show a broken entry.
fn info_file_for(item: &Path) -> Option<std::path::PathBuf> {
    let name = item.file_name()?.to_string_lossy();
    Some(item.with_file_name(format!("$I{}", name.strip_prefix("$R")?)))
}

fn remove_file_forced(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            // Read-only files cannot be deleted until the flag is cleared
            let mut perms = std::fs::metadata(path)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            std::fs::set_permissions(path, perms)?;
            std::fs::remove_file(path)
        }
        other => other,
    }
}

fn delete_item(path: &Path) -> Result<u64, String> {
    if !is_recycle_bin_item(path) {
        return Err("Not an item in the Recycle Bin".into());
    }
    let meta = std::fs::symlink_metadata(path).map_err(|_| "It is no longer in the Recycle Bin".to_string())?;
    let cancel = AtomicBool::new(false);
    let size = if meta.is_dir() {
        sizer::measure(path, &cancel).size_on_disk
    } else {
        meta.len()
    };
    // remove_dir_all does not follow junctions or symlinks; a linked folder only loses the link.
    let result = if meta.is_dir() && !crate::residual::roots::is_reparse_point(&meta) {
        std::fs::remove_dir_all(path)
    } else if meta.is_dir() {
        std::fs::remove_dir(path)
    } else {
        remove_file_forced(path)
    };
    result.map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => "Access denied. A program may be using it.".to_string(),
        _ => format!("Could not delete it: {}", e),
    })?;
    if let Some(info) = info_file_for(path) {
        let _ = remove_file_forced(&info);
    }
    Ok(size)
}

fn delete_blocking(paths: Vec<String>) -> Result<RecycleBinDeleteReport, String> {
    let mut deleted = 0u64;
    let mut freed_bytes = 0u64;
    let mut failed = Vec::new();
    for path in paths {
        match delete_item(Path::new(&path)) {
            Ok(size) => {
                deleted += 1;
                freed_bytes = freed_bytes.saturating_add(size);
            }
            Err(reason) => failed.push(RecycleBinDeleteFailure { path, reason }),
        }
    }
    Ok(RecycleBinDeleteReport {
        deleted,
        freed_bytes,
        failed,
        listing: list_blocking()?,
    })
}

#[tauri::command]
pub async fn recycle_bin_list() -> Result<RecycleBinListing, String> {
    tauri::async_runtime::spawn_blocking(list_blocking)
        .await
        .map_err(|e| format!("Recycle Bin thread error: {}", e))?
}

/// Permanently deletes the chosen Recycle Bin items. Each path is checked to be a bin item.
#[tauri::command]
pub async fn recycle_bin_delete(paths: Vec<String>) -> Result<RecycleBinDeleteReport, String> {
    tauri::async_runtime::spawn_blocking(move || delete_blocking(paths))
        .await
        .map_err(|e| format!("Recycle Bin thread error: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_recycle_bin_items() {
        assert!(is_recycle_bin_item(Path::new(r"C:\$Recycle.Bin\S-1-5-21-1-1001\$RZRH6RX")));
        assert!(is_recycle_bin_item(Path::new(r"D:\$RECYCLE.BIN\S-1-5-21-1-1001\$RAB12.pdf")));
        // The metadata file, the bin itself, a user folder, other paths
        assert!(!is_recycle_bin_item(Path::new(r"C:\$Recycle.Bin\S-1-5-21-1-1001\$IZRH6RX")));
        assert!(!is_recycle_bin_item(Path::new(r"C:\$Recycle.Bin\S-1-5-21-1-1001")));
        assert!(!is_recycle_bin_item(Path::new(r"C:\$Recycle.Bin\S-1-5-21-1-1001\$R")));
        assert!(!is_recycle_bin_item(Path::new(r"C:\$Recycle.Bin\S-1-5-21-1-1001\$RAB\inner")));
        assert!(!is_recycle_bin_item(Path::new(r"C:\$Recycle.Bin\S-1-5-21-1-1001\..\..\Windows")));
        assert!(!is_recycle_bin_item(Path::new(r"C:\Users\me\$Recycle.Bin\S-1-5\$RAB")));
        assert!(!is_recycle_bin_item(Path::new(r"C:\Windows\System32")));
        assert!(!is_recycle_bin_item(Path::new(r"$Recycle.Bin\S-1-5\$RAB")));
    }

    /// Recycles a temp folder, then deletes it forever. Touches the real Recycle Bin.
    /// `cargo test --lib recycle_bin_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn recycle_bin_live_delete_roundtrip() {
        let dir = crate::residual::tests::temp_dir("rb-live");
        let target = dir.join("asao-rb-live");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("a.txt"), b"hello").unwrap();
        crate::residual::cleaner::delete_folder(&target, false).unwrap();

        let listing = list_blocking().unwrap();
        let wanted = dir.to_string_lossy().to_lowercase();
        let item = listing
            .items
            .iter()
            .find(|i| i.name == "asao-rb-live" && i.original_folder.as_deref().map(|f| f.to_lowercase()) == Some(wanted.clone()))
            .expect("recycled folder is listed")
            .clone();
        println!("listed: {:?}", item);
        let info = info_file_for(Path::new(&item.path)).unwrap();
        assert!(info.exists(), "$I file exists before delete");

        let report = delete_blocking(vec![item.path.clone()]).unwrap();
        println!("deleted {}, freed {}, failed {:?}", report.deleted, report.freed_bytes, report.failed);
        assert_eq!(report.deleted, 1);
        assert!(!Path::new(&item.path).exists());
        assert!(!info.exists());
        assert!(report.listing.items.iter().all(|i| i.path != item.path));

        // Anything outside the bin is refused
        let outside = delete_blocking(vec![dir.to_string_lossy().to_string()]).unwrap();
        assert_eq!(outside.deleted, 0);
        assert!(dir.exists());
    }

    #[test]
    fn info_file_sits_next_to_item() {
        assert_eq!(
            info_file_for(Path::new(r"C:\$Recycle.Bin\S-1\$RAB12.pdf")).unwrap(),
            Path::new(r"C:\$Recycle.Bin\S-1\$IAB12.pdf")
        );
    }

    fn item(name: &str, path: &str, is_folder: bool) -> RecycleBinItem {
        RecycleBinItem {
            name: name.into(),
            path: path.into(),
            original_folder: None,
            is_folder,
            type_name: None,
            size_bytes: None,
            deleted_ms: None,
        }
    }

    #[test]
    fn restores_hidden_extension() {
        assert_eq!(display_name(&item("report", r"C:\$Recycle.Bin\S-1\$RAB12.pdf", false)), "report.pdf");
        assert_eq!(display_name(&item("report.PDF", r"C:\$Recycle.Bin\S-1\$RAB12.pdf", false)), "report.PDF");
        assert_eq!(display_name(&item("notes", r"C:\$Recycle.Bin\S-1\$RAB13", false)), "notes");
        assert_eq!(display_name(&item("My.Folder", r"C:\$Recycle.Bin\S-1\$RAB14.Folder", true)), "My.Folder");
    }
}
