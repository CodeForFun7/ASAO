use std::collections::VecDeque;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

#[derive(Debug, Clone, Copy)]
pub struct MemorySample {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f32,
    pub delta_percent: f32,
}

pub struct MemoryCollector {
    history: VecDeque<f32>,
}

impl MemoryCollector {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(20),
        }
    }

    pub fn sample(&mut self) -> MemorySample {
        unsafe {
            let mut mem_info = MEMORYSTATUSEX {
                dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                ..Default::default()
            };

            if GlobalMemoryStatusEx(&mut mem_info).is_ok() {
                let total = mem_info.ullTotalPhys;
                let avail = mem_info.ullAvailPhys;
                let used = total.saturating_sub(avail);
                let percent = if total > 0 {
                    ((used as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
                } else {
                    mem_info.dwMemoryLoad as f32
                };

                let avg = if self.history.is_empty() {
                    percent
                } else {
                    self.history.iter().sum::<f32>() / (self.history.len() as f32)
                };

                if self.history.len() >= 15 {
                    self.history.pop_front();
                }
                self.history.push_back(percent);

                let delta = ((percent - avg) * 10.0).round() / 10.0;
                let rounded_percent = (percent * 10.0).round() / 10.0;

                MemorySample {
                    total_bytes: total,
                    used_bytes: used,
                    usage_percent: rounded_percent,
                    delta_percent: delta,
                }
            } else {
                MemorySample {
                    total_bytes: 0,
                    used_bytes: 0,
                    usage_percent: 0.0,
                    delta_percent: 0.0,
                }
            }
        }
    }
}
