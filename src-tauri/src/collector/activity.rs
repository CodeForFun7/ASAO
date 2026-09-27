use std::collections::HashMap;
use std::time::{Duration, Instant};

#[repr(C)]
struct LastInputInfo {
    cb_size: u32,
    dw_time: u32,
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> isize;
    fn GetWindowThreadProcessId(hwnd: isize, lpdwprocessid: *mut u32) -> u32;
    fn GetLastInputInfo(plii: *mut LastInputInfo) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn GetTickCount() -> u32;
}

#[derive(Debug, Clone)]
pub struct UserActivitySample {
    pub foreground_pid: Option<u32>,
    pub user_idle_seconds: u64,
    pub is_user_active: bool,
}

pub struct UserActivityCollector {
    recent_foreground_pids: HashMap<u32, Instant>,
}

impl UserActivityCollector {
    pub fn new() -> Self {
        Self {
            recent_foreground_pids: HashMap::with_capacity(64),
        }
    }

    pub fn sample(&mut self) -> UserActivitySample {
        let now = Instant::now();
        let foreground_pid = query_foreground_pid();
        let user_idle_seconds = query_user_idle_seconds();

        if let Some(pid) = foreground_pid {
            if pid > 4 {
                self.recent_foreground_pids.insert(pid, now);
            }
        }

        // Prune entries older than 5 minutes to keep memory bounded
        self.recent_foreground_pids
            .retain(|_, last_seen| now.duration_since(*last_seen) < Duration::from_secs(300));

        UserActivitySample {
            foreground_pid,
            user_idle_seconds,
            is_user_active: user_idle_seconds < 60,
        }
    }

    /// Returns how many seconds ago `pid` was the user's active foreground window, if seen recently.
    pub fn seconds_since_foreground(&self, pid: u32, now: Instant) -> Option<u64> {
        self.recent_foreground_pids
            .get(&pid)
            .map(|last| now.duration_since(*last).as_secs())
    }
}

#[cfg(target_os = "windows")]
fn query_foreground_pid() -> Option<u32> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return None;
        }
        let mut pid: u32 = 0;
        let _thread_id = GetWindowThreadProcessId(hwnd, &mut pid);
        if pid > 0 {
            Some(pid)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn query_foreground_pid() -> Option<u32> {
    None
}

#[cfg(target_os = "windows")]
fn query_user_idle_seconds() -> u64 {
    unsafe {
        let mut lii = LastInputInfo {
            cb_size: std::mem::size_of::<LastInputInfo>() as u32,
            dw_time: 0,
        };
        if GetLastInputInfo(&mut lii) != 0 {
            let now_ticks = GetTickCount();
            let elapsed_ms = now_ticks.wrapping_sub(lii.dw_time);
            (elapsed_ms / 1000) as u64
        } else {
            0
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn query_user_idle_seconds() -> u64 {
    0
}
