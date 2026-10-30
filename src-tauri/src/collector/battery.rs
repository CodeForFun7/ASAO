//! Battery state from Windows: charge level and AC state (`GetSystemPowerStatus`), plus the
//! real charge/discharge rate, capacity, health and cycle count from the battery driver
//! (`IOCTL_BATTERY_QUERY_*`). Desktops without a battery report `has_battery: false`.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BatteryState {
    /// No battery (desktop), or Windows could not tell.
    NoBattery,
    Charging,
    Discharging,
    /// Plugged in and full, or plugged in but held below full by the charge limit.
    PluggedIn,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryStatus {
    pub has_battery: bool,
    pub state: BatteryState,
    pub ac_online: Option<bool>,
    pub percent: Option<u8>,
    /// Charge rate (positive) or discharge rate (negative) in watts, from the driver.
    pub rate_watts: Option<f64>,
    /// Time to empty while discharging, as estimated by Windows.
    pub time_remaining_secs: Option<u32>,
    /// Time to full while charging, from the remaining capacity and the charge rate.
    pub time_to_full_secs: Option<u32>,
    pub remaining_mwh: Option<u32>,
    pub full_charge_mwh: Option<u32>,
    pub design_mwh: Option<u32>,
    /// Full charge capacity as a share of the design capacity.
    pub health_percent: Option<u8>,
    pub cycle_count: Option<u32>,
    pub battery_saver: bool,
}

impl BatteryStatus {
    fn none() -> Self {
        Self {
            has_battery: false,
            state: BatteryState::NoBattery,
            ac_online: None,
            percent: None,
            rate_watts: None,
            time_remaining_secs: None,
            time_to_full_secs: None,
            remaining_mwh: None,
            full_charge_mwh: None,
            design_mwh: None,
            health_percent: None,
            cycle_count: None,
            battery_saver: false,
        }
    }
}

/// Driver readings summed over every battery (some laptops have two).
#[derive(Debug, Default, Clone)]
struct DriverReading {
    batteries: u32,
    rate_mw: Option<i64>,
    remaining_mwh: Option<u64>,
    full_charge_mwh: Option<u64>,
    design_mwh: Option<u64>,
    cycle_count: Option<u32>,
    charging: bool,
    discharging: bool,
}

pub fn read_battery_status() -> BatteryStatus {
    imp::read()
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{BatteryState, BatteryStatus, DriverReading};
    use std::mem::size_of;
    use windows::core::PCWSTR;
    use windows::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT,
        GUID_DEVCLASS_BATTERY, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
    };
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Power::{
        GetSystemPowerStatus, BATTERY_INFORMATION, BATTERY_QUERY_INFORMATION, BATTERY_STATUS,
        BATTERY_WAIT_STATUS, BatteryInformation, IOCTL_BATTERY_QUERY_INFORMATION,
        IOCTL_BATTERY_QUERY_STATUS, IOCTL_BATTERY_QUERY_TAG, SYSTEM_POWER_STATUS,
    };
    use windows::Win32::System::IO::DeviceIoControl;

    const BATTERY_UNKNOWN: u32 = 0xFFFF_FFFF;
    const BATTERY_UNKNOWN_RATE: i32 = i32::MIN;
    const BATTERY_CAPACITY_RELATIVE: u32 = 0x4000_0000;
    const BATTERY_DISCHARGING: u32 = 0x2;
    const BATTERY_CHARGING: u32 = 0x4;
    const FLAG_CHARGING: u8 = 8;
    const FLAG_NO_BATTERY: u8 = 128;
    const FLAG_UNKNOWN: u8 = 255;

    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    fn ioctl<I, O: Default>(h: HANDLE, code: u32, input: &I) -> Option<O> {
        let mut out = O::default();
        let mut returned = 0u32;
        unsafe {
            DeviceIoControl(
                h,
                code,
                Some(input as *const I as *const core::ffi::c_void),
                size_of::<I>() as u32,
                Some(&mut out as *mut O as *mut core::ffi::c_void),
                size_of::<O>() as u32,
                Some(&mut returned),
                None,
            )
            .ok()?;
        }
        Some(out)
    }

    fn known(v: u32) -> Option<u64> {
        if v == BATTERY_UNKNOWN || v == 0 {
            None
        } else {
            Some(v as u64)
        }
    }

    fn add(a: Option<u64>, b: Option<u64>) -> Option<u64> {
        match (a, b) {
            (Some(x), Some(y)) => Some(x + y),
            (x, None) => x,
            (None, y) => y,
        }
    }

    /// Reads one battery's driver data and adds it to `acc`.
    fn read_device(path: PCWSTR, acc: &mut DriverReading) -> Option<()> {
        let handle = unsafe {
            CreateFileW(
                path,
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
            .ok()?
        };
        let h = Handle(handle);

        let wait: u32 = 0;
        let tag: u32 = ioctl(h.0, IOCTL_BATTERY_QUERY_TAG, &wait)?;
        if tag == 0 {
            return None;
        }
        acc.batteries += 1;

        let query = BATTERY_QUERY_INFORMATION {
            BatteryTag: tag,
            InformationLevel: BatteryInformation,
            AtRate: 0,
        };
        let info: Option<BATTERY_INFORMATION> = ioctl(h.0, IOCTL_BATTERY_QUERY_INFORMATION, &query);
        // Relative batteries report percentages, not mWh. Their capacity and rate mean nothing in watts.
        let relative = info
            .map(|i| i.Capabilities & BATTERY_CAPACITY_RELATIVE != 0)
            .unwrap_or(false);
        if let Some(i) = info.filter(|_| !relative) {
            acc.full_charge_mwh = add(acc.full_charge_mwh, known(i.FullChargedCapacity));
            acc.design_mwh = add(acc.design_mwh, known(i.DesignedCapacity));
            if i.CycleCount > 0 {
                acc.cycle_count = Some(acc.cycle_count.unwrap_or(0).max(i.CycleCount));
            }
        }

        let wait_status = BATTERY_WAIT_STATUS {
            BatteryTag: tag,
            ..Default::default()
        };
        if let Some(st) = ioctl::<_, BATTERY_STATUS>(h.0, IOCTL_BATTERY_QUERY_STATUS, &wait_status) {
            if st.PowerState & BATTERY_CHARGING != 0 {
                acc.charging = true;
            }
            if st.PowerState & BATTERY_DISCHARGING != 0 {
                acc.discharging = true;
            }
            if !relative {
                acc.remaining_mwh = add(acc.remaining_mwh, known(st.Capacity));
                if st.Rate != BATTERY_UNKNOWN_RATE {
                    acc.rate_mw = Some(acc.rate_mw.unwrap_or(0) + st.Rate as i64);
                }
            }
        }
        Some(())
    }

    fn read_driver() -> DriverReading {
        let mut acc = DriverReading::default();
        unsafe {
            let Ok(set) = SetupDiGetClassDevsW(
                Some(&GUID_DEVCLASS_BATTERY),
                PCWSTR::null(),
                None,
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            ) else {
                return acc;
            };
            for index in 0..8u32 {
                let mut iface = SP_DEVICE_INTERFACE_DATA {
                    cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiEnumDeviceInterfaces(set, None, &GUID_DEVCLASS_BATTERY, index, &mut iface).is_err() {
                    break;
                }
                let mut needed = 0u32;
                let _ = SetupDiGetDeviceInterfaceDetailW(set, &iface, None, 0, Some(&mut needed), None);
                if needed == 0 {
                    continue;
                }
                // u64 storage keeps the detail struct aligned.
                let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
                let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
                (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
                if SetupDiGetDeviceInterfaceDetailW(set, &iface, Some(detail), needed, None, None).is_err() {
                    continue;
                }
                let path = PCWSTR((*detail).DevicePath.as_ptr());
                let _ = read_device(path, &mut acc);
            }
            let _ = SetupDiDestroyDeviceInfoList(set);
        }
        acc
    }

    pub fn read() -> BatteryStatus {
        let mut sps = SYSTEM_POWER_STATUS::default();
        if unsafe { GetSystemPowerStatus(&mut sps) }.is_err() {
            return BatteryStatus::none();
        }
        let driver = read_driver();
        let flag_says_none = sps.BatteryFlag != FLAG_UNKNOWN && sps.BatteryFlag & FLAG_NO_BATTERY != 0;
        if flag_says_none && driver.batteries == 0 {
            return BatteryStatus {
                ac_online: Some(true),
                ..BatteryStatus::none()
            };
        }

        let ac_online = match sps.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
        let percent = (sps.BatteryLifePercent <= 100).then_some(sps.BatteryLifePercent);
        let charging = driver.charging
            || (sps.BatteryFlag != FLAG_UNKNOWN && sps.BatteryFlag & FLAG_CHARGING != 0);
        let state = if charging {
            BatteryState::Charging
        } else if ac_online == Some(false) || driver.discharging {
            BatteryState::Discharging
        } else if ac_online == Some(true) {
            BatteryState::PluggedIn
        } else {
            BatteryState::Unknown
        };

        // Some drivers report a rate of zero while plugged in; only a non-zero rate is shown.
        let rate_watts = driver
            .rate_mw
            .filter(|r| *r != 0)
            .map(|r| (r as f64 / 1000.0 * 10.0).round() / 10.0);
        let time_remaining_secs = (state == BatteryState::Discharging && sps.BatteryLifeTime != u32::MAX)
            .then_some(sps.BatteryLifeTime);
        let time_to_full_secs = match (state, driver.rate_mw, driver.remaining_mwh, driver.full_charge_mwh) {
            (BatteryState::Charging, Some(rate), Some(rem), Some(full)) if rate > 0 && full > rem => {
                Some(((full - rem) as f64 / rate as f64 * 3600.0) as u32)
            }
            _ => None,
        };
        let health_percent = match (driver.full_charge_mwh, driver.design_mwh) {
            (Some(full), Some(design)) if design > 0 => Some(((full * 100) / design).min(100) as u8),
            _ => None,
        };

        BatteryStatus {
            has_battery: true,
            state,
            ac_online,
            percent,
            rate_watts,
            time_remaining_secs,
            time_to_full_secs,
            remaining_mwh: driver.remaining_mwh.map(|v| v as u32),
            full_charge_mwh: driver.full_charge_mwh.map(|v| v as u32),
            design_mwh: driver.design_mwh.map(|v| v as u32),
            health_percent,
            cycle_count: driver.cycle_count,
            battery_saver: sps.SystemStatusFlag == 1,
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn read() -> super::BatteryStatus {
        super::BatteryStatus::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read-only: prints what this machine reports.
    #[test]
    #[ignore]
    fn live_battery_status() {
        println!("{:#?}", read_battery_status());
    }
}
