use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::FILETIME,
    Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives},
    System::{
        Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::GetSystemTimes,
    },
};

use crate::system::{
    battery::{Battery, BatteryState},
    Disk, Jiffies, Memory, Metric, Readings,
};

#[derive(Default)]
pub struct SystemReadings {
    graphics: std::sync::Mutex<super::gpu::GraphicsReader>,
}

impl Readings for SystemReadings {
    fn reset_graphics(&self) {
        if let Ok(mut graphics) = self.graphics.lock() {
            graphics.reset();
        }
    }

    fn graphics(&self) -> Option<crate::system::Graphics> {
        Some(self.graphics.lock().map(|mut reader| reader.read()).unwrap_or(
            crate::system::Graphics { devices: Vec::new(), failed: true },
        ))
    }
    fn cpu(&self) -> Result<Jiffies, ()> {
        let (mut idle, mut kernel, mut user) = Default::default();
        unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }
            .map_err(|_| ())?;
        let ticks =
            |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
        // Kernel time already includes idle time.
        Ok(Jiffies::new(ticks(idle), ticks(kernel) + ticks(user)))
    }

    fn memory(&self) -> Result<(Memory, Option<Memory>), ()> {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        unsafe { GlobalMemoryStatusEx(&mut status) }.map_err(|_| ())?;
        let total = status.ullTotalPhys;
        if total == 0 {
            return Err(());
        }
        let used = total.checked_sub(status.ullAvailPhys).ok_or(())?;
        Ok((Memory { used, total }, None))
    }

    fn disks(&self) -> Result<Vec<Disk>, ()> {
        const DRIVE_FIXED: u32 = 3;
        let mask = unsafe { GetLogicalDrives() };
        let mut disks: Vec<_> = (b'A'..=b'Z')
            .filter(|letter| mask & (1 << (letter - b'A')) != 0)
            .filter_map(|letter| {
                let root = [u16::from(letter), u16::from(b':'), u16::from(b'\\'), 0];
                if unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } != DRIVE_FIXED {
                    return None;
                }
                let mount = format!("{}:", char::from(letter));
                let capacity = super::space(Path::new(&format!("{mount}\\"))).ok()?;
                // The capacity and available bytes returned to this caller account for quotas.
                let total = capacity.total;
                let free = capacity.available;
                let used = total.checked_sub(free)?;
                (total > 0).then_some(Disk {
                    mount,
                    used,
                    total,
                    free,
                    warning: u128::from(used) * 10 > u128::from(total) * 9,
                })
            })
            .collect();
        if disks.is_empty() {
            return Err(());
        }
        if let Ok(system) = std::env::var("SystemDrive") {
            disks.sort_by_key(|disk| !disk.mount.eq_ignore_ascii_case(&system));
        }
        Ok(disks)
    }

    fn battery(&self) -> Result<Option<Battery>, ()> {
        let mut status = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut status) }.map_err(|_| ())?;
        battery(status)
    }

    fn unsupported(&self) -> &[Metric] {
        &[Metric::Temperature, Metric::Swap]
    }
}

fn battery(status: SYSTEM_POWER_STATUS) -> Result<Option<Battery>, ()> {
    let percentage = status.BatteryLifePercent;
    let flags = status.BatteryFlag;
    if flags == 255 {
        return Err(());
    }
    if flags & 128 != 0 {
        return Ok(None);
    }
    if percentage > 100 {
        return Err(());
    }
    let state = match status.ACLineStatus {
        0 if percentage == 0 => BatteryState::Empty,
        0 => BatteryState::Discharging,
        1 if flags & 8 != 0 => BatteryState::Charging,
        1 if percentage == 100 => BatteryState::Full,
        1 => BatteryState::PluggedIn,
        _ => BatteryState::Unknown,
    };
    let seconds = status.BatteryLifeTime;
    Ok(Some(Battery {
        percentage: f64::from(percentage),
        state,
        seconds_remaining: (state == BatteryState::Discharging
            && seconds != u32::MAX
            && seconds > 0)
            .then_some(i64::from(seconds)),
        warning: percentage < 15,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_system_reads_cpu_ram_and_fixed_storage() {
        let source = SystemReadings::default();
        assert!(source.cpu().is_ok());
        let (ram, swap) = source.memory().unwrap();
        assert!(ram.total > 0 && ram.used <= ram.total);
        assert!(swap.is_none());
        assert!(source
            .disks()
            .unwrap()
            .iter()
            .all(|d| d.total > 0 && d.used + d.free == d.total));
    }

    #[test]
    fn absent_and_unreadable_batteries_have_distinct_states() {
        let mut status = SYSTEM_POWER_STATUS {
            BatteryFlag: 128,
            BatteryLifePercent: 255,
            ..Default::default()
        };
        assert_eq!(battery(status), Ok(None));
        status.BatteryFlag = 255;
        assert_eq!(battery(status), Err(()));
        status.BatteryFlag = 2;
        status.BatteryLifePercent = 12;
        status.BatteryLifeTime = 180;
        let reading = battery(status).unwrap().unwrap();
        assert_eq!(reading.state, BatteryState::Discharging);
        assert!(reading.warning);
        assert_eq!(reading.seconds_remaining, Some(180));
    }
}
