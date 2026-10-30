//! Fast NTFS scanner that reads the Master File Table (MFT) straight from the raw volume.
//!
//! Every file and folder on an NTFS volume has one record in the MFT holding its name,
//! parent folder, size and timestamps. Reading that table sequentially and rebuilding the
//! tree in memory avoids one filesystem call per directory, which is why this takes seconds.
//!
//! Opening `\\.\X:` requires administrator rights. Any problem before batches are emitted
//! (no admin rights, not NTFS, unexpected layout) is reported as `ScanFailure::Unavailable`
//! so the caller can fall back to the parallel directory walker.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::analyzer::storage_analyzer::{ScanBatch, ScanDirInfo, ScanFailure, ScanFileInfo};

const FILE_SHARE_ALL: u32 = 0x1 | 0x2 | 0x4;
const ROOT_RECORD: u64 = 5;
const FIRST_USER_RECORD: u64 = 24;
const REF_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const ATTR_REPARSE_POINT: u32 = 0x400;
const READ_CHUNK_BYTES: u64 = 8 * 1024 * 1024;

struct MftEntry {
    parent: u64,
    size: u64,
    created: u64,
    modified: u64,
    accessed: u64,
    attrs: u32,
    is_dir: bool,
    name: Box<str>,
}

fn u16le(b: &[u8], off: usize) -> u16 {
    b.get(off..off + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .unwrap_or(0)
}

fn u32le(b: &[u8], off: usize) -> u32 {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .unwrap_or(0)
}

fn u64le(b: &[u8], off: usize) -> u64 {
    b.get(off..off + 8)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
        .unwrap_or(0)
}

/// Windows FILETIME (100ns ticks since 1601) to Unix milliseconds.
fn filetime_to_ms(ft: u64) -> Option<u64> {
    if ft == 0 {
        return None;
    }
    (ft / 10_000).checked_sub(11_644_473_600_000)
}

fn unavailable<T>(msg: impl Into<String>) -> Result<T, ScanFailure> {
    Err(ScanFailure::Unavailable(msg.into()))
}

/// Undo the NTFS update-sequence protection on a record. Returns false if the record is torn.
fn apply_fixups(rec: &mut [u8]) -> bool {
    if rec.len() < 48 || &rec[0..4] != b"FILE" {
        return false;
    }
    let usa_off = u16le(rec, 4) as usize;
    let usa_cnt = u16le(rec, 6) as usize;
    if usa_cnt < 2 || usa_off + usa_cnt * 2 > rec.len() {
        return false;
    }
    for i in 1..usa_cnt {
        let end = i * 512;
        if end > rec.len() {
            break;
        }
        if rec[end - 2] != rec[usa_off] || rec[end - 1] != rec[usa_off + 1] {
            return false;
        }
        rec[end - 2] = rec[usa_off + i * 2];
        rec[end - 1] = rec[usa_off + i * 2 + 1];
    }
    true
}

/// Decode an NTFS data-run list into (start LCN, length in clusters). `None` start means sparse.
fn parse_runs(rec: &[u8], mut pos: usize, end: usize) -> Option<Vec<(Option<i64>, u64)>> {
    let mut runs = Vec::new();
    let mut lcn: i64 = 0;
    while pos < end.min(rec.len()) {
        let header = rec[pos];
        if header == 0 {
            break;
        }
        let len_bytes = (header & 0x0F) as usize;
        let off_bytes = (header >> 4) as usize;
        pos += 1;
        if len_bytes == 0 || len_bytes > 8 || off_bytes > 8 || pos + len_bytes + off_bytes > rec.len() {
            return None;
        }
        let mut len: u64 = 0;
        for i in 0..len_bytes {
            len |= (rec[pos + i] as u64) << (8 * i);
        }
        pos += len_bytes;
        if off_bytes == 0 {
            runs.push((None, len));
        } else {
            let mut delta: i64 = 0;
            for i in 0..off_bytes {
                delta |= (rec[pos + i] as i64) << (8 * i);
            }
            if rec[pos + off_bytes - 1] & 0x80 != 0 && off_bytes < 8 {
                delta |= -1i64 << (8 * off_bytes);
            }
            pos += off_bytes;
            lcn += delta;
            runs.push((Some(lcn), len));
        }
    }
    Some(runs)
}

struct VolumeLayout {
    cluster_size: u64,
    sector_size: u64,
    record_size: usize,
    mft_offset: u64,
}

fn read_exact_at(file: &mut File, offset: u64, len: usize) -> Result<Vec<u8>, ScanFailure> {
    let mut buf = vec![0u8; len];
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.read_exact(&mut buf))
        .map_err(|e| ScanFailure::Unavailable(format!("raw volume read failed: {e}")))?;
    Ok(buf)
}

fn read_layout(file: &mut File) -> Result<VolumeLayout, ScanFailure> {
    // 4096 bytes is sector aligned for both 512-byte and 4Kn drives.
    let boot = read_exact_at(file, 0, 4096)?;
    if &boot[3..11] != b"NTFS    " {
        return unavailable("volume is not NTFS");
    }
    let sector_size = u16le(&boot, 0x0B) as u64;
    let sectors_per_cluster = boot[0x0D] as u64;
    let mft_lcn = u64le(&boot, 0x30);
    let cpr = boot[0x40] as i8;
    if sector_size == 0 || sectors_per_cluster == 0 {
        return unavailable("invalid NTFS boot sector");
    }
    let cluster_size = sector_size * sectors_per_cluster;
    let record_size = if cpr > 0 {
        cpr as u64 * cluster_size
    } else {
        1u64 << (-(cpr as i32)) as u32
    } as usize;
    if record_size < 512 || record_size > 64 * 1024 {
        return unavailable("unexpected MFT record size");
    }
    Ok(VolumeLayout {
        cluster_size,
        sector_size,
        record_size,
        mft_offset: mft_lcn * cluster_size,
    })
}

/// Locate the extents and size of the $MFT file itself from its own record (record 0).
fn read_mft_extents(
    file: &mut File,
    layout: &VolumeLayout,
) -> Result<(Vec<(Option<i64>, u64)>, u64), ScanFailure> {
    let read_len = layout.record_size.max(layout.sector_size as usize);
    let mut buf = read_exact_at(file, layout.mft_offset, read_len)?;
    let rec = &mut buf[..layout.record_size];
    if !apply_fixups(rec) {
        return unavailable("$MFT record 0 is invalid");
    }
    let used = (u32le(rec, 0x18) as usize).min(rec.len());
    let mut off = u16le(rec, 0x14) as usize;
    while off + 16 <= used {
        let atype = u32le(rec, off);
        if atype == 0xFFFF_FFFF {
            break;
        }
        let alen = u32le(rec, off + 4) as usize;
        if alen < 16 || off + alen > used {
            break;
        }
        if atype == 0x80 && rec[off + 9] == 0 && rec[off + 8] != 0 {
            let real_size = u64le(rec, off + 0x30);
            let runs_off = off + u16le(rec, off + 0x20) as usize;
            let runs = parse_runs(rec, runs_off, off + alen)
                .ok_or_else(|| ScanFailure::Unavailable("bad $MFT data runs".into()))?;
            let covered: u64 = runs.iter().map(|(_, l)| l * layout.cluster_size).sum();
            if covered < real_size {
                return unavailable("$MFT is too fragmented to map from its first record");
            }
            return Ok((runs, real_size));
        }
        off += alen;
    }
    unavailable("$MFT data attribute not found")
}

fn parse_record(
    rec: &mut [u8],
    recno: u64,
    entries: &mut [Option<MftEntry>],
    ext_sizes: &mut HashMap<u64, u64>,
) {
    if !apply_fixups(rec) {
        return;
    }
    let flags = u16le(rec, 0x16);
    if flags & 1 == 0 {
        return;
    }
    let is_dir = flags & 2 != 0;
    let base = u64le(rec, 0x20) & REF_MASK;

    let used = (u32le(rec, 0x18) as usize).min(rec.len());
    let mut off = u16le(rec, 0x14) as usize;

    let (mut created, mut modified, mut accessed, mut attrs) = (0u64, 0u64, 0u64, 0u32);
    let mut best_name: Option<(u64, u8, String)> = None;
    let mut data_size: Option<u64> = None;

    while off + 16 <= used {
        let atype = u32le(rec, off);
        if atype == 0xFFFF_FFFF {
            break;
        }
        let alen = u32le(rec, off + 4) as usize;
        if alen < 16 || off + alen > used {
            break;
        }
        let non_resident = rec[off + 8] != 0;
        let name_len = rec[off + 9];

        match atype {
            0x10 if !non_resident => {
                let co = off + u16le(rec, off + 0x14) as usize;
                if co + 36 <= rec.len() {
                    created = u64le(rec, co);
                    modified = u64le(rec, co + 8);
                    accessed = u64le(rec, co + 24);
                    attrs = u32le(rec, co + 32);
                }
            }
            0x30 if !non_resident => {
                let co = off + u16le(rec, off + 0x14) as usize;
                if co + 0x42 <= rec.len() {
                    let parent = u64le(rec, co) & REF_MASK;
                    let chars = rec[co + 0x40] as usize;
                    let namespace = rec[co + 0x41];
                    let rank = match namespace {
                        1 | 3 => 3,
                        0 => 2,
                        _ => 1,
                    };
                    if best_name.as_ref().map(|(_, r, _)| rank > *r).unwrap_or(true)
                        && co + 0x42 + chars * 2 <= rec.len()
                    {
                        let units: Vec<u16> = rec[co + 0x42..co + 0x42 + chars * 2]
                            .chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        best_name = Some((parent, rank, String::from_utf16_lossy(&units)));
                    }
                }
            }
            0x80 if name_len == 0 => {
                if !non_resident {
                    data_size = Some(u32le(rec, off + 0x10) as u64);
                } else if u64le(rec, off + 0x10) == 0 {
                    data_size = Some(u64le(rec, off + 0x30));
                }
            }
            _ => {}
        }
        off += alen;
    }

    if base != 0 {
        // Extension record: only useful for the size of a fragmented file's $DATA stream.
        if let Some(sz) = data_size {
            ext_sizes.insert(base, sz);
        }
        return;
    }

    if recno < FIRST_USER_RECORD && recno != ROOT_RECORD {
        return;
    }
    let Some((parent, _rank, name)) = best_name else {
        return;
    };
    if let Some(slot) = entries.get_mut(recno as usize) {
        *slot = Some(MftEntry {
            parent,
            size: if is_dir { 0 } else { data_size.unwrap_or(0) },
            created,
            modified,
            accessed,
            attrs,
            is_dir,
            name: name.into_boxed_str(),
        });
    }
}

fn join_path(parent: &str, name: &str) -> String {
    if parent.ends_with('\\') {
        format!("{parent}{name}")
    } else {
        format!("{parent}\\{name}")
    }
}

/// Resolve the full path of directory record `rec`, or None if it is not reachable from the root.
fn resolve_dir_path(
    rec: u64,
    entries: &[Option<MftEntry>],
    memo: &mut HashMap<u64, Option<String>>,
    root: &str,
) -> Option<String> {
    let mut chain: Vec<u64> = Vec::new();
    let mut cur = rec;
    let base: Option<String> = loop {
        if let Some(p) = memo.get(&cur) {
            break p.clone();
        }
        if cur == ROOT_RECORD {
            let r = Some(root.to_string());
            memo.insert(cur, r.clone());
            break r;
        }
        match entries.get(cur as usize).and_then(|e| e.as_ref()) {
            Some(e) if e.is_dir && chain.len() < 512 => {
                chain.push(cur);
                cur = e.parent;
            }
            _ => break None,
        }
    };

    let mut path = base;
    for &r in chain.iter().rev() {
        path = path.and_then(|p| {
            entries[r as usize]
                .as_ref()
                .map(|e| join_path(&p, &e.name))
        });
        memo.insert(r, path.clone());
    }
    path
}

pub fn scan_volume(
    mount_point: &str,
    cancel: &AtomicBool,
    on_batch: &mut dyn FnMut(ScanBatch),
) -> Result<(), ScanFailure> {
    let letter = mount_point
        .chars()
        .next()
        .filter(|c| c.is_ascii_alphabetic())
        .ok_or_else(|| ScanFailure::Unavailable("not a drive-letter volume".into()))?;

    let mut file = match OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_ALL)
        .open(format!(r"\\.\{}:", letter.to_ascii_uppercase()))
    {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            return unavailable("administrator rights are required to read the MFT");
        }
        Err(e) => return unavailable(format!("cannot open volume: {e}")),
    };

    let layout = read_layout(&mut file)?;
    let (runs, mft_size) = read_mft_extents(&mut file, &layout)?;
    let record_size = layout.record_size;
    let total_records = (mft_size / record_size as u64) as usize;

    let mut entries: Vec<Option<MftEntry>> = Vec::new();
    entries.resize_with(total_records, || None);
    let mut ext_sizes: HashMap<u64, u64> = HashMap::new();

    // Stream the $MFT extent by extent, carrying partial records across read boundaries.
    let mut carry: Vec<u8> = Vec::new();
    let mut recno: u64 = 0;
    let mut remaining = mft_size.div_ceil(layout.sector_size) * layout.sector_size;
    for (lcn, len_clusters) in runs {
        let Some(lcn) = lcn else {
            return unavailable("sparse $MFT extent");
        };
        if lcn < 0 {
            return unavailable("negative LCN in $MFT extent");
        }
        let mut run_off = lcn as u64 * layout.cluster_size;
        let mut run_left = (len_clusters * layout.cluster_size).min(remaining);
        while run_left > 0 {
            if cancel.load(Ordering::Relaxed) {
                return Err(ScanFailure::Cancelled);
            }
            let n = run_left.min(READ_CHUNK_BYTES);
            let chunk = read_exact_at(&mut file, run_off, n as usize)?;
            run_off += n;
            run_left -= n;
            remaining -= n;

            let mut buf = std::mem::take(&mut carry);
            buf.extend_from_slice(&chunk);
            let whole = buf.len() / record_size;
            for i in 0..whole {
                let slice = &mut buf[i * record_size..(i + 1) * record_size];
                parse_record(slice, recno, &mut entries, &mut ext_sizes);
                recno += 1;
            }
            carry = buf[whole * record_size..].to_vec();
        }
        if remaining == 0 {
            break;
        }
    }

    // Sizes of files whose $DATA lives in an extension record.
    for (base, sz) in ext_sizes {
        if let Some(Some(e)) = entries.get_mut(base as usize) {
            if !e.is_dir && e.size == 0 {
                e.size = sz;
            }
        }
    }

    // Group children by parent so each folder becomes one batch.
    let mut order: Vec<u32> = entries
        .iter()
        .enumerate()
        .filter(|(i, e)| *i as u64 != ROOT_RECORD && e.is_some())
        .map(|(i, _)| i as u32)
        .collect();
    order.sort_unstable_by_key(|&i| entries[i as usize].as_ref().map(|e| e.parent).unwrap_or(0));

    let root = format!("{}:\\", letter.to_ascii_uppercase());
    let mut memo: HashMap<u64, Option<String>> = HashMap::new();

    let mut start = 0usize;
    let mut groups = 0usize;
    while start < order.len() {
        let parent = entries[order[start] as usize].as_ref().map(|e| e.parent).unwrap_or(0);
        let mut end = start;
        while end < order.len()
            && entries[order[end] as usize].as_ref().map(|e| e.parent) == Some(parent)
        {
            end += 1;
        }

        groups += 1;
        if groups % 512 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(ScanFailure::Cancelled);
        }

        if let Some(dir_path) = resolve_dir_path(parent, &entries, &mut memo, &root) {
            let mut batch = ScanBatch {
                dir_path: dir_path.clone(),
                dirs: Vec::new(),
                files: Vec::new(),
            };
            for &idx in &order[start..end] {
                let Some(e) = entries[idx as usize].as_ref() else {
                    continue;
                };
                let path = join_path(&dir_path, &e.name);
                if e.is_dir {
                    if e.attrs & ATTR_REPARSE_POINT != 0 {
                        continue;
                    }
                    batch.dirs.push(ScanDirInfo {
                        name: e.name.to_string(),
                        path,
                        created_ms: filetime_to_ms(e.created),
                        modified_ms: filetime_to_ms(e.modified),
                        accessed_ms: filetime_to_ms(e.accessed),
                    });
                } else {
                    batch.files.push(ScanFileInfo {
                        name: e.name.to_string(),
                        path,
                        size: e.size,
                        created_ms: filetime_to_ms(e.created),
                        modified_ms: filetime_to_ms(e.modified),
                        accessed_ms: filetime_to_ms(e.accessed),
                    });
                }
            }
            on_batch(batch);
        }
        start = end;
    }

    Ok(())
}
