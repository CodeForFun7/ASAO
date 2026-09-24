use std::collections::VecDeque;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
use windows::Win32::System::Threading::GetSystemTimes;

#[inline]
pub fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

pub fn get_logical_cpu_count() -> u32 {
    unsafe {
        let mut sys_info = SYSTEM_INFO::default();
        GetSystemInfo(&mut sys_info);
        sys_info.dwNumberOfProcessors.max(1)
    }
}

pub struct CpuCollector {
    prev_idle: u64,
    prev_kernel: u64,
    prev_user: u64,
    history: VecDeque<f32>,
}

impl CpuCollector {
    pub fn new() -> Self {
        let (idle, kernel, user) = Self::read_system_times().unwrap_or((0, 0, 0));
        Self {
            prev_idle: idle,
            prev_kernel: kernel,
            prev_user: user,
            history: VecDeque::with_capacity(20),
        }
    }

    fn read_system_times() -> Option<(u64, u64, u64)> {
        unsafe {
            let mut idle = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                Some((
                    filetime_to_u64(idle),
                    filetime_to_u64(kernel),
                    filetime_to_u64(user),
                ))
            } else {
                None
            }
        }
    }

    /// Returns `(current_cpu_percent, delta_from_recent_average, total_system_delta_100ns)`
    pub fn sample(&mut self) -> (f32, f32, u64) {
        let Some((idle, kernel, user)) = Self::read_system_times() else {
            return (0.0, 0.0, 10_000_000);
        };

        let idle_delta = idle.saturating_sub(self.prev_idle);
        let kernel_delta = kernel.saturating_sub(self.prev_kernel);
        let user_delta = user.saturating_sub(self.prev_user);

        self.prev_idle = idle;
        self.prev_kernel = kernel;
        self.prev_user = user;

        let total_sys_delta = kernel_delta.saturating_add(user_delta);
        let active_delta = total_sys_delta.saturating_sub(idle_delta);

        let usage = if total_sys_delta > 0 {
            ((active_delta as f64 / total_sys_delta as f64) * 100.0)
                .clamp(0.0, 100.0) as f32
        } else {
            0.0
        };

        let avg = if self.history.is_empty() {
            usage
        } else {
            self.history.iter().sum::<f32>() / (self.history.len() as f32)
        };

        if self.history.len() >= 15 {
            self.history.pop_front();
        }
        self.history.push_back(usage);

        let delta = ((usage - avg) * 10.0).round() / 10.0;
        let rounded_usage = (usage * 10.0).round() / 10.0;

        (rounded_usage, delta, total_sys_delta.max(1))
    }
}
