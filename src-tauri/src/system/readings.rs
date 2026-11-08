//! Read /proc and /sys through the filesystem port.

use super::{battery::Battery, Metric};

use serde::Serialize;

#[cfg(any(target_os = "linux", test))]
use crate::ports::Files;
#[cfg(any(target_os = "linux", test))]
use std::io;

pub trait Readings: Send {
    fn reset_graphics(&self) {}
    #[cfg(any(windows, test))]
    fn graphics(&self) -> Option<super::Graphics> {
        None
    }
    fn cpu(&self) -> Result<Jiffies, ()>;
    fn memory(&self) -> Result<(Memory, Option<Memory>), ()>;
    fn disks(&self) -> Result<Vec<Disk>, ()>;
    fn battery(&self) -> Result<Option<Battery>, ()>;
    fn temperature(&self) -> Option<Temperature> {
        None
    }
    fn unsupported(&self) -> &[Metric] {
        &[]
    }
    fn poll_battery(&self) -> bool {
        true
    }
}

#[cfg(any(target_os = "linux", test))]
pub struct LinuxReadings<A, U>(pub A, pub U);

#[cfg(any(target_os = "linux", test))]
impl<A: Files, U: super::battery::UPower> Readings for LinuxReadings<A, U> {
    fn cpu(&self) -> Result<Jiffies, ()> {
        jiffies(&self.0).map_err(|_| ())
    }
    fn memory(&self) -> Result<(Memory, Option<Memory>), ()> {
        memory(&self.0).map_err(|_| ())
    }
    fn disks(&self) -> Result<Vec<Disk>, ()> {
        Ok(disks(&self.0))
    }
    fn battery(&self) -> Result<Option<Battery>, ()> {
        Ok(super::battery::read(&self.1))
    }
    fn temperature(&self) -> Option<Temperature> {
        temperature(&self.0)
    }
    fn poll_battery(&self) -> bool {
        false
    }
}

/// Cumulative CPU times in the source's native units.
#[derive(Debug, Clone, Copy)]
pub struct Jiffies {
    idle: u64,
    total: u64,
}

impl Jiffies {
    #[cfg(windows)]
    pub fn new(idle: u64, total: u64) -> Self {
        Self { idle, total }
    }
    /// Percentage between two samples; None if counters have not advanced.
    pub fn usage_since(self, previous: Jiffies) -> Option<f64> {
        let total = self.total.checked_sub(previous.total).filter(|&t| t > 0)?;
        let idle = self.idle.checked_sub(previous.idle)?.min(total);
        Some((total - idle) as f64 * 100.0 / total as f64)
    }
}

#[cfg(any(target_os = "linux", test))]
fn jiffies(files: &impl Files) -> io::Result<Jiffies> {
    let stat = files.read("/proc/stat")?;
    let fields: Vec<u64> = stat
        .lines()
        .find_map(|l| l.strip_prefix("cpu "))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "/proc/stat has no cpu line"))?
        .split_whitespace()
        .map_while(|c| c.parse().ok())
        .collect();
    // user nice system idle iowait irq softirq steal; guest and guest_nice are already included in user/nice.
    if fields.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "/proc/stat curto",
        ));
    }
    Ok(Jiffies {
        idle: fields[3] + fields.get(4).unwrap_or(&0),
        total: fields.iter().take(8).sum(),
    })
}

/// In bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Memory {
    pub used: u64,
    pub total: u64,
}

/// RAM follows free: used = total minus MemAvailable; include swap when configured.
#[cfg(any(target_os = "linux", test))]
fn memory(files: &impl Files) -> io::Result<(Memory, Option<Memory>)> {
    let meminfo = files.read("/proc/meminfo")?;
    let field = |name: &str| -> io::Result<u64> {
        meminfo
            .lines()
            .find_map(|l| l.strip_prefix(name)?.strip_prefix(':'))
            .and_then(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())
            .map(|kb| kb * 1024)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, name.to_owned()))
    };
    let total = field("MemTotal")?;
    let ram = Memory {
        used: total.saturating_sub(field("MemAvailable")?),
        total,
    };
    let swap_total = field("SwapTotal").unwrap_or(0);
    let swap = (swap_total > 0).then(|| Memory {
        used: swap_total.saturating_sub(field("SwapFree").unwrap_or(swap_total)),
        total: swap_total,
    });
    Ok((ram, swap))
}

#[cfg(any(target_os = "linux", test))]
const CPU_SENSORS: [&str; 2] = ["k10temp", "coretemp"];
/// Labels representing the whole CPU, in preference order.
#[cfg(any(target_os = "linux", test))]
const CPU_LABELS: [&str; 3] = ["Tctl", "Tdie", "Package id 0"];
/// Warn at this temperature even if the sensor critical threshold is higher, in millidegrees Celsius.
#[cfg(any(target_os = "linux", test))]
const WARNING_THRESHOLD: i64 = 90_000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Temperature {
    /// One decimal place.
    pub celsius: f64,
    /// Reached the sensor critical threshold or 90 degrees Celsius.
    pub warning: bool,
}

/// CPU temperature through hwmon (k10temp or coretemp); None without a sensor.
#[cfg(any(target_os = "linux", test))]
fn temperature(files: &impl Files) -> Option<Temperature> {
    let mut sensors = files.list("/sys/class/hwmon").ok()?;
    sensors.sort();
    let dir = sensors
        .into_iter()
        .map(|h| format!("/sys/class/hwmon/{h}"))
        .find(|d| {
            files
                .read(&format!("{d}/name"))
                .is_ok_and(|n| CPU_SENSORS.contains(&n.trim()))
        })?;
    let read = |file: String| -> Option<String> {
        Some(files.read(&format!("{dir}/{file}")).ok()?.trim().to_owned())
    };

    let mut indices: Vec<u32> = files
        .list(&dir)
        .ok()?
        .iter()
        .filter_map(|a| a.strip_prefix("temp")?.strip_suffix("_input")?.parse().ok())
        .collect();
    indices.sort();
    let label = |i: &u32| read(format!("temp{i}_label")).unwrap_or_default();
    let index = CPU_LABELS
        .iter()
        .find_map(|r| indices.iter().find(|i| label(i) == *r))
        .or(indices.first())?;

    let millidegrees: i64 = read(format!("temp{index}_input"))?.parse().ok()?;
    let threshold = read(format!("temp{index}_crit"))
        .and_then(|c| c.parse().ok())
        .map_or(WARNING_THRESHOLD, |critical: i64| {
            critical.min(WARNING_THRESHOLD)
        });
    Some(Temperature {
        celsius: (millidegrees as f64 / 100.0).round() / 10.0,
        warning: millidegrees >= threshold,
    })
}

/// Filesystems that never represent a user disk, even on block devices.
#[cfg(any(target_os = "linux", test))]
const TYPES_EXCLUDED: [&str; 4] = ["tmpfs", "squashfs", "overlay", "efivarfs"];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Disk {
    pub mount: String,
    /// Bytes, as reported by df: free means available to the user.
    pub used: u64,
    pub total: u64,
    pub free: u64,
    /// More than 90% used.
    pub warning: bool,
}

/// One bars per mounted physical disk: filesystems on /dev/*, excluding snaps and /boot*.
/// One bars per block device, using its first mount (usually /). Btrfs subvolumes and bind
/// mounts repeat the source with a different anonymous major:minor.
#[cfg(any(target_os = "linux", test))]
fn disks(files: &impl Files) -> Vec<Disk> {
    let Ok(mountinfo) = files.read("/proc/self/mountinfo") else {
        return Vec::new();
    };
    let mut seen = Vec::new();
    mountinfo
        .lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let before: Vec<&str> = before.split(' ').collect();
            let mount = unescape(before.get(4)?);
            let mut after = after.split(' ');
            let (kind, source) = (after.next()?, after.next()?);
            let real = source.starts_with("/dev/")
                && !TYPES_EXCLUDED.contains(&kind)
                && !mount.starts_with("/boot")
                && !mount.starts_with("/snap/");
            if !real || seen.contains(&source) {
                return None;
            }
            seen.push(source);
            let space = files.space(&mount).ok().filter(|e| e.total > 0)?;
            let used = space.total.saturating_sub(space.free);
            Some(Disk {
                warning: used * 10 > (used + space.available) * 9,
                mount,
                used,
                total: space.total,
                free: space.available,
            })
        })
        .collect()
}

/// The kernel escapes spaces, tabs, newlines and backslashes as octal sequences such as \040.
#[cfg(any(target_os = "linux", test))]
fn unescape(field: &str) -> String {
    let mut output = String::with_capacity(field.len());
    let mut remainder = field;
    while let Some(i) = remainder.find('\\') {
        output.push_str(&remainder[..i]);
        let code = remainder.get(i + 1..i + 4);
        match code.and_then(|c| u8::from_str_radix(c, 8).ok()) {
            Some(byte) => {
                output.push(byte as char);
                remainder = &remainder[i + 4..];
            }
            None => {
                output.push('\\');
                remainder = &remainder[i + 1..];
            }
        }
    }
    output.push_str(remainder);
    output
}
