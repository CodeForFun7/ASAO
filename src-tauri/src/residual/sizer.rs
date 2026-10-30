//! `FolderSizer`: size on disk and newest modified time, without following reparse points.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::roots::{is_reparse_point, system_time_ms};

#[derive(Debug, Clone, Copy, Default)]
pub struct FolderSize {
    pub size_on_disk: u64,
    pub newest_modified_ms: Option<u64>,
}

#[cfg(target_os = "windows")]
fn file_size_on_disk(path: &Path, logical: u64) -> u64 {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetCompressedFileSizeW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut high = 0u32;
    let low = unsafe { GetCompressedFileSizeW(PCWSTR(wide.as_ptr()), Some(&mut high)) };
    if low == u32::MAX && high == 0 {
        // INVALID_FILE_SIZE: fall back to the logical size.
        return logical;
    }
    ((high as u64) << 32) | low as u64
}

#[cfg(not(target_os = "windows"))]
fn file_size_on_disk(_path: &Path, logical: u64) -> u64 {
    logical
}

pub fn measure(path: &Path, cancel: &AtomicBool) -> FolderSize {
    let mut out = FolderSize::default();
    let mut stack: Vec<PathBuf> = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            let Ok(meta) = fs::symlink_metadata(&p) else { continue };
            if is_reparse_point(&meta) {
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
                continue;
            }
            out.size_on_disk = out
                .size_on_disk
                .saturating_add(file_size_on_disk(&p, meta.len()));
            if let Some(m) = system_time_ms(meta.modified()) {
                if out.newest_modified_ms.map(|n| m > n).unwrap_or(true) {
                    out.newest_modified_ms = Some(m);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_nested_files() {
        let dir = crate::residual::tests::temp_dir("sizer");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.bin"), vec![1u8; 5000]).unwrap();
        fs::write(dir.join("sub").join("b.bin"), vec![1u8; 5000]).unwrap();
        let r = measure(&dir, &AtomicBool::new(false));
        // Size on disk depends on cluster size and compression, so only check it is counted.
        assert!(r.size_on_disk > 0, "got {}", r.size_on_disk);
        assert!(r.newest_modified_ms.is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
