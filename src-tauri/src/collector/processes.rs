use std::collections::HashMap;
use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::Storage::FileSystem::{
    GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    GetProcessIoCounters, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, IO_COUNTERS,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::collector::cpu::filetime_to_u64;

#[derive(Debug, Clone)]
pub struct RawProcessRecord {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub executable_path: Option<String>,
    pub publisher: Option<String>,
    pub product_name: Option<String>,
    pub file_description: Option<String>,
    pub thread_count: u32,
    pub memory_bytes: u64,
    pub total_cpu_time_100ns: u64,
    pub disk_io_bytes: u64,
    pub other_io_bytes: u64,
    pub started_seconds_ago: Option<u64>,
    pub is_restricted: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PeMetadata {
    pub publisher: Option<String>,
    pub product_name: Option<String>,
    pub description: Option<String>,
}

pub struct ProcessCollector {
    metadata_cache: HashMap<String, PeMetadata>,
}

impl ProcessCollector {
    pub fn new() -> Self {
        Self {
            metadata_cache: HashMap::with_capacity(256),
        }
    }

    pub fn collect_all(&mut self) -> Result<Vec<RawProcessRecord>, String> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(|e| format!("Failed to create ToolHelp32 snapshot: {}", e))?;

            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            let mut results = Vec::with_capacity(256);
            let now_filetime_100ns = current_time_as_filetime_100ns();

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    // Skip System Idle Process (PID 0)
                    if pid != 0 {
                        let name = wide_slice_to_string(&entry.szExeFile);
                        let parent_pid = Some(entry.th32ParentProcessID);
                        let thread_count = entry.cntThreads;

                        let record = self.inspect_single_process(
                            pid,
                            parent_pid,
                            name,
                            thread_count,
                            now_filetime_100ns,
                        );
                        results.push(record);
                    }

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
            Ok(results)
        }
    }

    unsafe fn inspect_single_process(
        &mut self,
        pid: u32,
        parent_pid: Option<u32>,
        name: String,
        thread_count: u32,
        now_filetime_100ns: u64,
    ) -> RawProcessRecord {
        let handle_res = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);

        let Ok(handle) = handle_res else {
            // Process is protected/elevated (e.g. System, Registry, PPL services)
            return RawProcessRecord {
                pid,
                parent_pid,
                name,
                executable_path: None,
                publisher: None,
                product_name: None,
                file_description: None,
                thread_count,
                memory_bytes: 0,
                total_cpu_time_100ns: 0,
                disk_io_bytes: 0,
                other_io_bytes: 0,
                started_seconds_ago: None,
                is_restricted: true,
            };
        };

        // 1. Query full executable path
        let mut path_buf = [0u16; 1024];
        let mut path_len = path_buf.len() as u32;
        let executable_path = if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(path_buf.as_mut_ptr()),
            &mut path_len,
        )
        .is_ok()
            && path_len > 0
        {
            Some(String::from_utf16_lossy(&path_buf[..path_len as usize]))
        } else {
            None
        };

        // 2. Query working set memory
        let mut mem_counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        let memory_bytes = if K32GetProcessMemoryInfo(
            handle,
            &mut mem_counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .as_bool()
        {
            mem_counters.WorkingSetSize as u64
        } else {
            0
        };

        // 3. Query process CPU times & start time
        let mut creation_ft = FILETIME::default();
        let mut exit_ft = FILETIME::default();
        let mut kernel_ft = FILETIME::default();
        let mut user_ft = FILETIME::default();

        let (total_cpu_time_100ns, started_seconds_ago) = if GetProcessTimes(
            handle,
            &mut creation_ft,
            &mut exit_ft,
            &mut kernel_ft,
            &mut user_ft,
        )
        .is_ok()
        {
            let cpu_total = filetime_to_u64(kernel_ft).saturating_add(filetime_to_u64(user_ft));
            let created_100ns = filetime_to_u64(creation_ft);
            let started_ago = if now_filetime_100ns > created_100ns && created_100ns > 0 {
                Some((now_filetime_100ns - created_100ns) / 10_000_000)
            } else {
                None
            };
            (cpu_total, started_ago)
        } else {
            (0, None)
        };

        // 4. Query process I/O counters (Disk Read+Write and Other/Network I/O)
        let mut io_counters = IO_COUNTERS::default();
        let (disk_io_bytes, other_io_bytes) =
            if GetProcessIoCounters(handle, &mut io_counters).is_ok() {
                (
                    io_counters
                        .ReadTransferCount
                        .saturating_add(io_counters.WriteTransferCount),
                    io_counters.OtherTransferCount,
                )
            } else {
                (0, 0)
            };

        let _ = CloseHandle(handle);

        // 5. Lookup PE Version Info metadata from cache or disk
        let meta = if let Some(ref path) = executable_path {
            if let Some(cached) = self.metadata_cache.get(path) {
                cached.clone()
            } else {
                let extracted = extract_pe_version_metadata(path);
                self.metadata_cache.insert(path.clone(), extracted.clone());
                extracted
            }
        } else {
            PeMetadata::default()
        };

        RawProcessRecord {
            pid,
            parent_pid,
            name,
            executable_path,
            publisher: meta.publisher,
            product_name: meta.product_name,
            file_description: meta.description,
            thread_count,
            memory_bytes,
            total_cpu_time_100ns,
            disk_io_bytes,
            other_io_bytes,
            started_seconds_ago,
            is_restricted: false,
        }
    }
}

fn wide_slice_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    OsString::from_wide(&buf[..end])
        .to_string_lossy()
        .into_owned()
}

fn current_time_as_filetime_100ns() -> u64 {
    // Windows epoch starts 1601-01-01T00:00:00Z (11644473600 seconds before Unix epoch)
    const EPOCH_DIFF_SECS: u64 = 11_644_473_600;
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => (dur.as_secs() + EPOCH_DIFF_SECS) * 10_000_000 + (dur.subsec_nanos() as u64 / 100),
        Err(_) => 0,
    }
}

unsafe fn extract_pe_version_metadata(path: &str) -> PeMetadata {
    let wide_path: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let size = GetFileVersionInfoSizeW(PCWSTR(wide_path.as_ptr()), None);
    if size == 0 {
        return PeMetadata::default();
    }

    let mut buffer = vec![0u8; size as usize];
    if GetFileVersionInfoW(
        PCWSTR(wide_path.as_ptr()),
        0,
        size,
        buffer.as_mut_ptr() as *mut _,
    )
    .is_err()
    {
        return PeMetadata::default();
    }

    // Read translation table (language + codepage)
    let trans_query: Vec<u16> = "\\VarFileInfo\\Translation\0".encode_utf16().collect();
    let mut trans_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut trans_len: u32 = 0;

    let mut lang_cp_candidates = Vec::with_capacity(3);
    if VerQueryValueW(
        buffer.as_ptr() as *const _,
        PCWSTR(trans_query.as_ptr()),
        &mut trans_ptr,
        &mut trans_len,
    )
    .as_bool()
        && !trans_ptr.is_null()
        && trans_len >= 4
    {
        let lang_cp = std::slice::from_raw_parts(trans_ptr as *const u16, 2);
        lang_cp_candidates.push(format!("{:04x}{:04x}", lang_cp[0], lang_cp[1]));
    }
    lang_cp_candidates.push("040904b0".to_string());
    lang_cp_candidates.push("040904e4".to_string());

    let query_field = |field_name: &str| -> Option<String> {
        for prefix in &lang_cp_candidates {
            let sub_block = format!("\\StringFileInfo\\{}\\{}\0", prefix, field_name);
            let wide_sub: Vec<u16> = sub_block.encode_utf16().collect();
            let mut val_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
            let mut val_len: u32 = 0;

            if VerQueryValueW(
                buffer.as_ptr() as *const _,
                PCWSTR(wide_sub.as_ptr()),
                &mut val_ptr,
                &mut val_len,
            )
            .as_bool()
                && !val_ptr.is_null()
                && val_len > 0
            {
                let slice = std::slice::from_raw_parts(val_ptr as *const u16, val_len as usize);
                let s = wide_slice_to_string(slice).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    };

    PeMetadata {
        publisher: query_field("CompanyName"),
        product_name: query_field("ProductName"),
        description: query_field("FileDescription"),
    }
}
