//! Slow-changing inputs of the SSI: startup items, drive capacity, storage composition and
//! boot history. A background thread refreshes them every 30 seconds; the 1 Hz loop only reads
//! the cached values.

use std::os::windows::process::CommandExt;
use std::process::Command;

use crate::analyzer::storage_analyzer::{normalize_key, StorageAnalyzerEngine};
use crate::collector::storage::StorageDrive;
use crate::startup::scanner::{estimate_item_delay_ms, ItemClass, StartupImpact, StartupItem, StartupSource};

use super::model::{CompositionSlice, DriveInfo, StartupTopItem, StorageComposition};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct StartupSummary {
    pub total_items: usize,
    /// Enabled startup applications that are not essential Windows components.
    pub enabled_optional: usize,
    pub high_impact: usize,
    /// Low impact counts 1, medium 2.5, high 5.
    pub weighted_count: f32,
    pub estimated_delay_ms: u64,
    pub top_items: Vec<StartupTopItem>,
}

fn is_user_facing(source: &StartupSource) -> bool {
    matches!(
        source,
        StartupSource::RegistryRunUser
            | StartupSource::RegistryRunMachine
            | StartupSource::RegistryRunOnceUser
            | StartupSource::RegistryRunOnceMachine
            | StartupSource::StartupFolderUser
            | StartupSource::StartupFolderMachine
            | StartupSource::ScheduledTask
            | StartupSource::PolicyRun
    )
}

pub fn summarize_startup(items: &[StartupItem]) -> StartupSummary {
    let optional: Vec<&StartupItem> = items
        .iter()
        .filter(|i| i.is_enabled && is_user_facing(&i.source) && i.classification != ItemClass::Essential)
        .collect();

    let mut weighted = 0.0f32;
    let mut high = 0usize;
    let mut delay = 0u64;
    for item in &optional {
        weighted += match item.impact {
            StartupImpact::Low => 1.0,
            StartupImpact::Medium => 2.5,
            StartupImpact::High => 5.0,
        };
        if item.impact == StartupImpact::High {
            high += 1;
        }
        delay = delay.saturating_add(estimate_item_delay_ms(item));
    }

    let mut ranked: Vec<(&StartupItem, u64)> = optional.iter().map(|i| (*i, estimate_item_delay_ms(i))).collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    let top_items = ranked
        .into_iter()
        .take(3)
        .map(|(item, delay_ms)| StartupTopItem {
            id: item.id.clone(),
            name: item.name.clone(),
            impact: match item.impact {
                StartupImpact::Low => "low",
                StartupImpact::Medium => "medium",
                StartupImpact::High => "high",
            }
            .to_string(),
            delay_ms,
        })
        .collect();

    StartupSummary {
        total_items: items.len(),
        enabled_optional: optional.len(),
        high_impact: high,
        weighted_count: weighted,
        estimated_delay_ms: delay,
        top_items,
    }
}

// ---------------------------------------------------------------------------
// Boot history
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum BootState {
    /// Not read yet.
    Pending,
    Available {
        latest_ms: u64,
        /// Median of earlier boots, when at least three exist.
        baseline_ms: Option<u64>,
        samples: usize,
    },
    Unavailable(String),
}

impl Default for BootState {
    fn default() -> Self {
        BootState::Pending
    }
}

/// Reads Windows' own boot-performance events (Diagnostics-Performance, event 100).
/// That log is restricted to administrators on most PCs.
pub fn read_boot_state() -> BootState {
    let output = Command::new("wevtutil")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "qe",
            "Microsoft-Windows-Diagnostics-Performance/Operational",
            "/q:*[System[(EventID=100)]]",
            "/c:15",
            "/rd:true",
            "/f:xml",
        ])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => return BootState::Unavailable(format!("Windows event log tool could not start: {}", e)),
    };
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_lowercase();
        return BootState::Unavailable(if err.contains("denied") {
            "Windows shares boot history with administrators only. Run ASAO as administrator to include it.".into()
        } else {
            "Windows boot history could not be read on this PC.".into()
        });
    }

    let boots = parse_boot_times(&String::from_utf8_lossy(&output.stdout));
    summarize_boots(&boots)
}

/// Boot durations in milliseconds, newest first.
pub fn parse_boot_times(xml: &str) -> Vec<u64> {
    let marker = "Name='BootTime'>";
    xml.split("<Event ")
        .skip(1)
        .filter_map(|event| {
            let start = event.find(marker)? + marker.len();
            let digits: String = event[start..].chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse::<u64>().ok().filter(|ms| *ms > 0)
        })
        .collect()
}

pub fn summarize_boots(boots_newest_first: &[u64]) -> BootState {
    let Some(&latest) = boots_newest_first.first() else {
        return BootState::Unavailable("Windows has not recorded a boot-performance event yet.".into());
    };
    let earlier = &boots_newest_first[1..];
    let baseline = (earlier.len() >= 3).then(|| {
        let mut sorted = earlier.to_vec();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    });
    BootState::Available { latest_ms: latest, baseline_ms: baseline, samples: boots_newest_first.len() }
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

pub fn system_drive() -> String {
    std::env::var("SystemDrive")
        .ok()
        .map(|d| d.trim_end_matches('\\').to_uppercase())
        .filter(|d| d.len() == 2 && d.ends_with(':'))
        .unwrap_or_else(|| "C:".to_string())
}

pub fn drives_from(collected: Vec<StorageDrive>) -> Vec<DriveInfo> {
    let system = system_drive();
    collected
        .into_iter()
        .filter(|d| d.drive_type == "Local Disk")
        .map(|d| DriveInfo {
            is_system: d.drive.eq_ignore_ascii_case(&system),
            drive: d.drive,
            name: d.name,
            total_bytes: d.total_capacity,
            used_bytes: d.used_capacity,
            free_bytes: d.free_capacity,
            used_percent: d.usage_percentage as f32,
        })
        .collect()
}

fn known_folder(id: &windows::core::GUID) -> Option<String> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};

    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, HANDLE::default()).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
        s
    }
}

/// Splits the used space of a scanned drive into the categories shown on the dashboard.
/// Returns `None` until a storage scan of that drive has finished in this session.
pub fn composition_from_engine(
    engine: &StorageAnalyzerEngine,
    drive: &DriveInfo,
    scanned_at_ms: u64,
) -> Option<StorageComposition> {
    use windows::Win32::UI::Shell::{
        FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_LocalAppData, FOLDERID_Music,
        FOLDERID_Pictures, FOLDERID_ProgramData, FOLDERID_ProgramFiles, FOLDERID_ProgramFilesX86,
        FOLDERID_RoamingAppData, FOLDERID_Videos, FOLDERID_Windows,
    };

    let key = drive.drive.trim_end_matches('\\').to_uppercase();
    let index = engine.indices.get(&key)?;
    let sizes = &index.folder_sizes;
    if sizes.is_empty() {
        return None;
    }

    let size_of = |path: &Option<String>| -> u64 {
        path.as_deref()
            .map(|p| sizes.get(&normalize_key(p)).copied().unwrap_or(0))
            .unwrap_or(0)
    };
    let is_inside = |child: &str, parent: &Option<String>| -> bool {
        parent
            .as_deref()
            .map(|p| normalize_key(child).starts_with(&format!("{}\\", normalize_key(p))))
            .unwrap_or(false)
    };

    let windows_dir = known_folder(&FOLDERID_Windows);
    let local_app = known_folder(&FOLDERID_LocalAppData);
    let user_temp = std::env::var("TEMP").ok().filter(|t| !t.is_empty());
    let windows_temp = windows_dir.as_ref().map(|w| format!("{}\\Temp", w.trim_end_matches('\\')));

    let user_temp_opt = user_temp.clone();
    let windows_temp_opt = windows_temp.clone();
    let temp_user_bytes = size_of(&user_temp_opt);
    let temp_windows_bytes = size_of(&windows_temp_opt);

    // Temp folders sit inside other folders that are counted too; subtract them there.
    let local_app_bytes = size_of(&local_app);
    let local_app_without_temp = if user_temp.as_deref().is_some_and(|t| is_inside(t, &local_app)) {
        local_app_bytes.saturating_sub(temp_user_bytes)
    } else {
        local_app_bytes
    };
    let windows_bytes = size_of(&windows_dir);
    let windows_without_temp = if windows_temp.is_some() {
        windows_bytes.saturating_sub(temp_windows_bytes)
    } else {
        windows_bytes
    };

    let applications = size_of(&known_folder(&FOLDERID_ProgramFiles))
        + size_of(&known_folder(&FOLDERID_ProgramFilesX86))
        + size_of(&known_folder(&FOLDERID_ProgramData))
        + size_of(&known_folder(&FOLDERID_RoamingAppData))
        + local_app_without_temp;
    let documents = size_of(&known_folder(&FOLDERID_Documents)) + size_of(&known_folder(&FOLDERID_Desktop));
    let downloads = size_of(&known_folder(&FOLDERID_Downloads));
    let media = size_of(&known_folder(&FOLDERID_Pictures))
        + size_of(&known_folder(&FOLDERID_Videos))
        + size_of(&known_folder(&FOLDERID_Music));
    let temporary = temp_user_bytes + temp_windows_bytes;
    let system = windows_without_temp;

    let mut parts = vec![
        ("applications", "Applications", applications),
        ("documents", "Documents", documents),
        ("downloads", "Downloads", downloads),
        ("media", "Media", media),
        ("temporary", "Temporary files", temporary),
        ("system", "System files", system),
    ];

    let known: u64 = parts.iter().map(|p| p.2).sum();
    if known == 0 {
        return None;
    }
    // Sizes come from a file scan, the used space from the volume. If the scan found more than
    // the volume reports (hard links, compression), scale down so the bar never overflows.
    let used = drive.used_bytes.max(1);
    if known > used {
        let scale = used as f64 / known as f64;
        for part in parts.iter_mut() {
            part.2 = (part.2 as f64 * scale) as u64;
        }
    }
    let known_after: u64 = parts.iter().map(|p| p.2).sum();
    let mut slices: Vec<CompositionSlice> = parts
        .into_iter()
        .map(|(id, label, bytes)| CompositionSlice { id: id.into(), label: label.into(), bytes })
        .collect();
    slices.push(CompositionSlice {
        id: "other".into(),
        label: "Other files".into(),
        bytes: used.saturating_sub(known_after),
    });

    Some(StorageComposition { drive: drive.drive.clone(), scanned_at_ms, slices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_boot_times_from_event_xml() {
        let xml = "<Event xmlns='x'><System><Provider Name='p'/></System><EventData>\
                   <Data Name='BootTime'>41239</Data><Data Name='MainPathBootTime'>30000</Data></EventData></Event>\
                   <Event xmlns='x'><EventData><Data Name='BootTime'>52000</Data></EventData></Event>";
        assert_eq!(parse_boot_times(xml), vec![41239, 52000]);
    }

    #[test]
    fn baseline_needs_three_earlier_boots() {
        assert!(matches!(
            summarize_boots(&[50_000, 40_000, 41_000]),
            BootState::Available { baseline_ms: None, .. }
        ));
        match summarize_boots(&[80_000, 40_000, 42_000, 38_000, 90_000]) {
            BootState::Available { latest_ms, baseline_ms, samples } => {
                assert_eq!(latest_ms, 80_000);
                assert_eq!(baseline_ms, Some(42_000));
                assert_eq!(samples, 5);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(summarize_boots(&[]), BootState::Unavailable(_)));
    }
}
