use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDrive {
    pub drive: String,
    pub mount_point: String,
    pub name: String,
    pub file_system: String,
    pub drive_type: String,
    pub total_capacity: u64,
    pub used_capacity: u64,
    pub free_capacity: u64,
    pub usage_percentage: f64,
}

#[cfg(target_os = "windows")]
pub fn collect_storage_drives() -> Vec<StorageDrive> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    };

    let mut drives = Vec::new();
    let bitmask = unsafe { GetLogicalDrives() };
    if bitmask == 0 {
        return drives;
    }

    for i in 0..26u32 {
        if (bitmask & (1 << i)) == 0 {
            continue;
        }

        let letter = (b'A' + i as u8) as char;
        let drive_label = format!("{}:", letter);
        let mount_point = format!("{}:\\", letter);

        let mut wide_path: Vec<u16> = mount_point.encode_utf16().collect();
        wide_path.push(0);
        let pcwstr = PCWSTR(wide_path.as_ptr());

        let drive_type_code = unsafe { GetDriveTypeW(pcwstr) };
        // Skip nonexistent or unmounted optical drives unless media is present
        let drive_type_str = match drive_type_code {
            2 => "Removable",
            3 => "Local Disk",
            4 => "Network Drive",
            5 => "Optical Drive",
            6 => "RAM Disk",
            _ => "Storage Volume",
        };

        let mut free_bytes_available: u64 = 0;
        let mut total_number_of_bytes: u64 = 0;
        let mut total_number_of_free_bytes: u64 = 0;

        let space_ok = unsafe {
            GetDiskFreeSpaceExW(
                pcwstr,
                Some(&mut free_bytes_available),
                Some(&mut total_number_of_bytes),
                Some(&mut total_number_of_free_bytes),
            )
            .is_ok()
        };

        if !space_ok || total_number_of_bytes == 0 {
            continue;
        }

        let mut vol_name_buf = [0u16; 261];
        let mut fs_name_buf = [0u16; 261];
        let mut serial_number = 0u32;
        let mut max_component_len = 0u32;
        let mut fs_flags = 0u32;

        let vol_ok = unsafe {
            GetVolumeInformationW(
                pcwstr,
                Some(&mut vol_name_buf),
                Some(&mut serial_number),
                Some(&mut max_component_len),
                Some(&mut fs_flags),
                Some(&mut fs_name_buf),
            )
            .is_ok()
        };

        let mut volume_name = String::new();
        let mut file_system = String::from("NTFS");

        if vol_ok {
            let vol_end = vol_name_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(vol_name_buf.len());
            volume_name = String::from_utf16_lossy(&vol_name_buf[..vol_end])
                .trim()
                .to_string();

            let fs_end = fs_name_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(fs_name_buf.len());
            let parsed_fs = String::from_utf16_lossy(&fs_name_buf[..fs_end])
                .trim()
                .to_string();
            if !parsed_fs.is_empty() {
                file_system = parsed_fs;
            }
        }

        if volume_name.is_empty() {
            volume_name = drive_type_str.to_string();
        }

        let free_capacity = total_number_of_free_bytes.min(total_number_of_bytes);
        let used_capacity = total_number_of_bytes.saturating_sub(free_capacity);
        let usage_percentage = if total_number_of_bytes > 0 {
            ((used_capacity as f64 / total_number_of_bytes as f64) * 1000.0).round() / 10.0
        } else {
            0.0
        };

        drives.push(StorageDrive {
            drive: drive_label,
            mount_point,
            name: volume_name,
            file_system,
            drive_type: drive_type_str.to_string(),
            total_capacity: total_number_of_bytes,
            used_capacity,
            free_capacity,
            usage_percentage,
        });
    }

    drives
}

#[cfg(not(target_os = "windows"))]
pub fn collect_storage_drives() -> Vec<StorageDrive> {
    Vec::new()
}

pub fn system_time_to_ms(time: std::io::Result<SystemTime>) -> Option<u64> {
    let t = time.ok()?;
    let dur = t.duration_since(UNIX_EPOCH).ok()?;
    Some(dur.as_millis() as u64)
}

pub fn normalize_win_path(p: &Path) -> String {
    let raw = p.to_string_lossy().to_string();
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        raw
    }
}
