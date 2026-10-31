use std::collections::VecDeque;

#[link(name = "pdh")]
extern "system" {
    fn PdhOpenQueryW(szDataSource: *const u16, dwUserData: usize, phQuery: *mut isize) -> u32;
    fn PdhAddEnglishCounterW(
        hQuery: isize,
        szFullCounterPath: *const u16,
        dwUserData: usize,
        phCounter: *mut isize,
    ) -> u32;
    fn PdhCollectQueryData(hQuery: isize) -> u32;
    fn PdhGetFormattedCounterArrayW(
        hCounter: isize,
        dwFormat: u32,
        lpdwBufferSize: *mut u32,
        lpdwItemCount: *mut u32,
        ItemBuffer: *mut PdhFmtCounterValueItemW,
    ) -> u32;
    fn PdhCloseQuery(hQuery: isize) -> u32;
}

const PDH_FMT_DOUBLE: u32 = 0x00000200;
const PDH_MORE_DATA: u32 = 0x800007D2;

#[repr(C)]
struct PdhFmtCounterValue {
    c_status: u32,
    double_value: f64,
}

#[repr(C)]
struct PdhFmtCounterValueItemW {
    sz_name: *const u16,
    fmt_value: PdhFmtCounterValue,
}

#[derive(Debug, Clone, Copy)]
pub struct GpuSample {
    pub usage_percent: f32,
    pub delta_percent: f32,
}

pub struct GpuCollector {
    query_handle: isize,
    counter_handle: isize,
    pdh_active: bool,
    history: VecDeque<f32>,
}

unsafe impl Send for GpuCollector {}
unsafe impl Sync for GpuCollector {}

impl GpuCollector {
    pub fn new() -> Self {
        let mut query_handle: isize = 0;
        let mut counter_handle: isize = 0;
        let mut pdh_active = false;

        unsafe {
            if PdhOpenQueryW(std::ptr::null(), 0, &mut query_handle) == 0 && query_handle != 0 {
                let path: Vec<u16> = "\\GPU Engine(*engtype_3D)\\Utilization Percentage\0"
                    .encode_utf16()
                    .collect();
                if PdhAddEnglishCounterW(query_handle, path.as_ptr(), 0, &mut counter_handle) == 0
                    && counter_handle != 0
                {
                    let _ = PdhCollectQueryData(query_handle);
                    pdh_active = true;
                }
            }
        }

        Self {
            query_handle,
            counter_handle,
            pdh_active,
            history: VecDeque::with_capacity(20),
        }
    }

    /// GPU use from Windows' "GPU Engine" 3D counters. `None` when the counters are not
    /// available on this PC. An idle GPU reads 0%, which is a real value.
    pub fn sample(&mut self) -> Option<GpuSample> {
        if !self.pdh_active {
            return None;
        }
        let mut total_3d = 0.0f64;
        unsafe {
            if PdhCollectQueryData(self.query_handle) != 0 {
                return None;
            }
            let mut buf_size: u32 = 0;
            let mut item_count: u32 = 0;
            let status = PdhGetFormattedCounterArrayW(
                self.counter_handle,
                PDH_FMT_DOUBLE,
                &mut buf_size,
                &mut item_count,
                std::ptr::null_mut(),
            );
            if (status == PDH_MORE_DATA || status == 0) && buf_size > 0 && item_count > 0 {
                let mut buffer = vec![0u8; buf_size as usize];
                let items_ptr = buffer.as_mut_ptr() as *mut PdhFmtCounterValueItemW;
                if PdhGetFormattedCounterArrayW(
                    self.counter_handle,
                    PDH_FMT_DOUBLE,
                    &mut buf_size,
                    &mut item_count,
                    items_ptr,
                ) == 0
                {
                    let items = std::slice::from_raw_parts(items_ptr, item_count as usize);
                    for item in items {
                        if item.fmt_value.c_status == 0 {
                            total_3d += item.fmt_value.double_value;
                        }
                    }
                }
            }
        }

        let rounded = ((total_3d.clamp(0.0, 100.0) as f32) * 10.0).round() / 10.0;
        let avg = if self.history.is_empty() {
            rounded
        } else {
            self.history.iter().sum::<f32>() / (self.history.len() as f32)
        };
        if self.history.len() >= 15 {
            self.history.pop_front();
        }
        self.history.push_back(rounded);

        Some(GpuSample {
            usage_percent: rounded,
            delta_percent: ((rounded - avg) * 10.0).round() / 10.0,
        })
    }
}

impl Drop for GpuCollector {
    fn drop(&mut self) {
        if self.query_handle != 0 {
            unsafe {
                let _ = PdhCloseQuery(self.query_handle);
            }
        }
    }
}
