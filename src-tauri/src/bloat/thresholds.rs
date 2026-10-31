//! Every threshold of the impact score. Tune here only.

/// Default CPU sampling window, in seconds.
pub const SAMPLE_SECONDS: u32 = 30;
pub const SAMPLE_INTERVAL_MS: u64 = 2_000;

// Startup (max 30), using Windows' own buckets.
pub const STARTUP_MAX: u32 = 30;
pub const STARTUP_HIGH_POINTS: u32 = 30;
pub const STARTUP_MEDIUM_POINTS: u32 = 15;
pub const STARTUP_LOW_POINTS: u32 = 5;
pub const STARTUP_HIGH_CPU_MS: u64 = 1_000;
pub const STARTUP_HIGH_DISK_BYTES: u64 = 3 * 1024 * 1024;
pub const STARTUP_MEDIUM_CPU_MS: u64 = 300;
pub const STARTUP_MEDIUM_DISK_BYTES: u64 = 300 * 1024;

// Background CPU (max 25): average % of the whole machine, highest bucket first.
pub const CPU_MAX: u32 = 25;
pub const CPU_STEPS: &[(f64, u32)] = &[(5.0, 25), (2.0, 18), (1.0, 12), (0.3, 6), (0.05, 2)];

// Memory (max 20): working set, highest bucket first.
pub const MEMORY_MAX: u32 = 20;
pub const MEMORY_STEPS: &[(u64, u32)] = &[
    (500 * 1024 * 1024, 20),
    (200 * 1024 * 1024, 15),
    (100 * 1024 * 1024, 10),
    (30 * 1024 * 1024, 5),
    (1, 2),
];

// Automatic services and frequent scheduled tasks (max 10), by count.
pub const AUTOSTART_MAX: u32 = 10;
pub const AUTOSTART_STEPS: &[(u32, u32)] = &[(3, 10), (2, 7), (1, 4)];

// Disk footprint (max 15), highest bucket first.
pub const DISK_MAX: u32 = 15;
pub const DISK_STEPS: &[(u64, u32)] = &[
    (2 * 1024 * 1024 * 1024, 15),
    (500 * 1024 * 1024, 10),
    (100 * 1024 * 1024, 6),
    (20 * 1024 * 1024, 3),
    (1, 1),
];

// Bands on the 0-100 total.
pub const BAND_HIGH: u32 = 60;
pub const BAND_MEDIUM: u32 = 30;

/// "Used recently" for the Caution downgrade.
pub const RECENT_USE_DAYS: u64 = 30;
