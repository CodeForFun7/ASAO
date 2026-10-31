//! System-wide network throughput from the network adapters' own byte counters
//! (`GetIfTable2`). Only physical adapters that are up are counted, so traffic that passes
//! through a virtual adapter (VPN, Hyper-V, WSL) on its way to a real one is not counted twice.

use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkSample {
    pub download_bytes_per_sec: u64,
    pub upload_bytes_per_sec: u64,
}

pub struct NetworkCollector {
    last: Option<(u64, u64, Instant)>,
}

impl NetworkCollector {
    pub fn new() -> Self {
        Self { last: None }
    }

    /// Bytes per second since the previous call. `None` when the adapters cannot be read;
    /// zero on the first call (there is nothing to compare with yet).
    pub fn sample(&mut self) -> Option<NetworkSample> {
        let (rx, tx) = read_totals()?;
        let now = Instant::now();
        let out = match self.last {
            Some((prev_rx, prev_tx, at)) => {
                let secs = now.duration_since(at).as_secs_f64().max(0.001);
                NetworkSample {
                    download_bytes_per_sec: (rx.saturating_sub(prev_rx) as f64 / secs) as u64,
                    upload_bytes_per_sec: (tx.saturating_sub(prev_tx) as f64 / secs) as u64,
                }
            }
            None => NetworkSample::default(),
        };
        self.last = Some((rx, tx, now));
        Some(out)
    }
}

/// Total bytes received and sent by physical adapters that are up.
#[cfg(target_os = "windows")]
fn read_totals() -> Option<(u64, u64)> {
    use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;

    const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).is_err() || table.is_null() {
            return None;
        }
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let (mut rx, mut tx) = (0u64, 0u64);
        for r in rows {
            // Bit 0 of the flags: HardwareInterface.
            let hardware = r.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if !hardware || r.Type == IF_TYPE_SOFTWARE_LOOPBACK || r.OperStatus != IfOperStatusUp {
                continue;
            }
            rx = rx.saturating_add(r.InOctets);
            tx = tx.saturating_add(r.OutOctets);
        }
        FreeMibTable(table as *const core::ffi::c_void);
        Some((rx, tx))
    }
}

#[cfg(not(target_os = "windows"))]
fn read_totals() -> Option<(u64, u64)> {
    None
}
