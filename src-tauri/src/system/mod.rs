//! System module: CPU, temperature, RAM, disks and battery, collected only while visible.

pub(crate) mod battery;
mod readings;

use std::collections::VecDeque;
use std::sync::mpsc::Sender;
#[cfg(target_os = "linux")]
use std::sync::mpsc::{self, RecvTimeoutError};
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::Duration;

use serde::Serialize;
#[cfg(target_os = "linux")]
use tauri::AppHandle;

use crate::module::Module;
use crate::ports::{Clock, Files};
#[cfg(target_os = "linux")]
use crate::ports::{RootFilesystem, SystemClock};
#[cfg(target_os = "linux")]
use crate::snapshot;
use crate::snapshot::Snapshot;
use battery::Battery;
#[cfg(test)]
use battery::BatteryState;
pub use battery::UPower;
#[cfg(target_os = "linux")]
use battery::UPowerDbus;
use readings::{Disk, Jiffies, Memory, Temperature};

const CPU_INTERVAL: u64 = 2_000;
const DISK_INTERVAL: u64 = 30_000;
/// About 60 seconds of history, sampled every 2 seconds.
const HISTORY_POINTS: usize = 30;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemData {
    pub cpu: Cpu,
    pub ram: Memory,
    pub swap: Option<Memory>,
    pub disks: Vec<Disk>,
    /// None without a battery.
    pub battery: Option<Battery>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Cpu {
    /// Total usage percentage with one decimal place; None until two samples exist.
    pub usage: Option<f64>,
    /// Oldest to newest.
    pub history: Vec<f64>,
    /// None without a CPU sensor.
    pub temperature: Option<Temperature>,
}

/// System collector. The background loop calls step when the deadline expires.
pub struct System<A, U, R> {
    files: A,
    upower: U,
    clock: R,
    visible: bool,
    next_cpu: u64,
    next_disks: u64,
    disks: Vec<Disk>,
    jiffies: Option<Jiffies>,
    usage: Option<f64>,
    history: VecDeque<f64>,
    battery: Option<Battery>,
    last: Option<SystemData>,
}

impl<A: Files, U: UPower, R: Clock> System<A, U, R> {
    pub fn new(files: A, upower: U, clock: R) -> Self {
        System {
            files,
            upower,
            clock,
            visible: false,
            next_cpu: 0,
            next_disks: 0,
            disks: Vec::new(),
            jiffies: None,
            usage: None,
            history: VecDeque::new(),
            battery: None,
            last: None,
        }
    }

    /// The module became visible: restart collection with empty history.
    pub fn show(&mut self) {
        if self.visible {
            return;
        }
        self.visible = true;
        self.jiffies = None;
        self.usage = None;
        self.history.clear();
        self.battery = battery::read(&self.upower);
        self.next_cpu = self.clock.now();
        self.next_disks = self.next_cpu;
    }

    pub fn resume(&mut self) {
        if self.visible {
            self.hide();
            self.show();
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.last = None;
    }

    /// UPower reported a DisplayDevice change.
    pub fn battery_changed(&mut self) -> Option<(u64, SystemData)> {
        if !self.visible {
            return None;
        }
        self.battery = battery::read(&self.upower);
        let mut data = self.last.clone()?;
        data.battery = self.battery.clone();
        self.last = Some(data.clone());
        Some((self.clock.now(), data))
    }

    /// The next collection deadline; None while the module is hidden.
    pub fn deadline(&self) -> Option<u64> {
        self.visible.then_some(self.next_cpu.min(self.next_disks))
    }

    /// Collect due readings and return a snapshot with the collection time in epoch milliseconds.
    pub fn step(&mut self) -> Option<(u64, SystemData)> {
        let now = self.clock.now();
        if !self.visible || self.deadline().is_some_and(|p| now < p) {
            return None;
        }
        if now >= self.next_disks {
            self.next_disks = now + DISK_INTERVAL;
            self.disks = readings::disks(&self.files);
        }
        if now >= self.next_cpu {
            self.next_cpu = now + CPU_INTERVAL;
            self.usage = self.measure_cpu();
        }
        let (ram, swap) = readings::memory(&self.files).ok()?;
        let data = SystemData {
            cpu: Cpu {
                usage: self.usage,
                history: self.history.iter().copied().collect(),
                temperature: readings::temperature(&self.files),
            },
            ram,
            swap,
            disks: self.disks.clone(),
            battery: self.battery.clone(),
        };
        self.last = Some(data.clone());
        Some((now, data))
    }

    fn measure_cpu(&mut self) -> Option<f64> {
        let current = readings::jiffies(&self.files).ok();
        let previous = std::mem::replace(&mut self.jiffies, current);
        let usage = (current?.usage_since(previous?)? * 10.0).round() / 10.0;
        if self.history.len() == HISTORY_POINTS {
            self.history.pop_front();
        }
        self.history.push_back(usage);
        Some(usage)
    }
}

enum Event {
    Visible(bool),
    BatteryChanged,
    Resume,
}

/// Channel for the collector thread, which sleeps until a deadline or event.
pub struct Collector(Sender<Event>);

impl Collector {
    #[cfg(target_os = "linux")]
    pub fn start(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        let notice = tx.clone();
        battery::listen_upower(move || {
            let _ = notice.send(Event::BatteryChanged);
        });
        let loop_handle = move || {
            let clock = SystemClock;
            let mut system =
                System::new(RootFilesystem::system(), UPowerDbus::connect(), SystemClock);
            loop {
                let event = match system.deadline() {
                    Some(deadline) => {
                        rx.recv_timeout(Duration::from_millis(deadline.saturating_sub(clock.now())))
                    }
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                let next = match event {
                    Ok(Event::Visible(true)) => {
                        system.show();
                        None
                    }
                    Ok(Event::Visible(false)) => {
                        system.hide();
                        None
                    }
                    Ok(Event::Resume) => {
                        system.resume();
                        None
                    }
                    Ok(Event::BatteryChanged) => system.battery_changed(),
                    Err(RecvTimeoutError::Timeout) => system.step(),
                    Err(RecvTimeoutError::Disconnected) => return,
                };
                if let Some((when, data)) = next {
                    snapshot::publish(&app, snapshot(when, &data));
                }
            }
        };
        if let Err(e) = thread::Builder::new()
            .name("system".into())
            .spawn(loop_handle)
        {
            eprintln!("system: failed to start collection: {e}");
        }
        Collector(tx)
    }

    /// Fixture mode: no collection; events are dropped.
    #[cfg(feature = "fixtures")]
    pub fn detached() -> Self {
        Collector(mpsc::channel().0)
    }

    pub fn resume(&self) {
        let _ = self.0.send(Event::Resume);
    }

    /// Collect only while the System module is visible.
    pub fn report(&self, visible: bool) {
        let _ = self.0.send(Event::Visible(visible));
    }
}

fn snapshot(when: u64, data: &SystemData) -> Snapshot {
    Snapshot {
        module: Module::System,
        updated_at: Some(when),
        data: serde_json::to_value(data).expect("SystemData must serialize"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::fakes::{FakeClock, FakeRoot};
    use crate::ports::Space;

    const GIB: u64 = 1024 * 1024 * 1024;
    const KIB: u64 = 1024;

    struct Scenario {
        root: FakeRoot,
        upower: battery::fake::FakeUPower,
        clock: FakeClock,
        system: System<FakeRoot, battery::fake::FakeUPower, FakeClock>,
    }

    fn notebook() -> Scenario {
        let root = FakeRoot::from("system/notebook");
        // `stat -f -c "%n %S %b %f %a"` recorded alongside the fixture.
        let statvfs = std::fs::read_to_string(
            crate::ports::fakes::fixtures().join("system/notebook-statvfs.txt"),
        )
        .unwrap();
        for line in statvfs.lines() {
            let c: Vec<&str> = line.split_whitespace().collect();
            let n = |i: usize| c[i].parse::<u64>().unwrap();
            root.set_space(
                c[0],
                Space {
                    total: n(2) * n(1),
                    free: n(3) * n(1),
                    available: n(4) * n(1),
                },
            );
        }
        let upower = battery::fake::FakeUPower::from("notebook-charged");
        let clock = FakeClock::at(1_000_000);
        let system = System::new(root.clone(), upower.clone(), clock.clone());
        Scenario {
            root,
            upower,
            clock,
            system,
        }
    }

    #[test]
    fn resume_discards_cpu_baseline_and_collects_without_waiting() {
        let mut c = notebook();
        c.system.show();
        c.system.step().unwrap();
        c.clock.advance(2_000);
        c.system.step().unwrap();
        c.system.resume();
        let data = c.system.step().unwrap().1;
        assert_eq!(data.cpu.usage, None);
        assert!(data.cpu.history.is_empty());
        c.system.hide();
        c.system.resume();
        assert!(c.system.step().is_none());
    }

    #[test]
    fn ram_usage_is_total_minus_available_like_free() {
        let mut c = notebook();
        c.system.show();

        let (when, data) = c.system.step().unwrap();

        assert_eq!(when, 1_000_000);
        // MemTotal 15223128 kB, MemAvailable 9252764 kB.
        assert_eq!(data.ram.total, 15_223_128 * KIB);
        assert_eq!(data.ram.used, (15_223_128 - 9_252_764) * KIB);
        assert!(data.ram.total > 14 * GIB);
    }

    #[test]
    fn configured_swap_is_shown() {
        let mut c = notebook();
        c.system.show();

        let swap = c.system.step().unwrap().1.swap.unwrap();

        // SwapTotal 4194300 kB, SwapFree 3801600 kB.
        assert_eq!(swap.total, 4_194_300 * KIB);
        assert_eq!(swap.used, (4_194_300 - 3_801_600) * KIB);
    }

    #[test]
    fn unconfigured_swap_is_hidden() {
        let mut c = notebook();
        let meminfo = c.root.read("/proc/meminfo").unwrap();
        c.root.write(
            "/proc/meminfo",
            &meminfo
                .replace("SwapTotal:       4194300 kB", "SwapTotal:             0 kB")
                .replace("SwapFree:        3801600 kB", "SwapFree:              0 kB"),
        );
        c.system.show();

        assert_eq!(c.system.step().unwrap().1.swap, None);
    }

    fn sample_cpu(c: &Scenario, n: usize) {
        let stat = std::fs::read_to_string(
            crate::ports::fakes::fixtures().join(format!("system/proc-stat/{n}")),
        )
        .unwrap();
        c.root.write("/proc/stat", &stat);
    }

    #[test]
    fn cpu_starts_empty_on_open() {
        let mut c = notebook();
        c.system.show();

        let cpu = c.system.step().unwrap().1.cpu;

        assert_eq!(cpu.usage, None);
        assert!(cpu.history.is_empty());
    }

    #[test]
    fn cpu_uses_sample_deltas_every_two_seconds() {
        let mut c = notebook();
        c.system.show();
        sample_cpu(&c, 0);
        c.system.step().unwrap();
        assert_eq!(c.system.deadline(), Some(1_002_000));

        sample_cpu(&c, 1);
        c.clock.advance(1_000);
        assert_eq!(c.system.step(), None, "nada venceu em 1 s");
        c.clock.advance(1_000);
        let (when, data) = c.system.step().unwrap();

        // 0 to 1: busy +55 jiffies (user 44, system 10, softirq 1) out of +2395 total.
        assert_eq!(when, 1_002_000);
        assert_eq!(data.cpu.usage, Some(2.3));
        assert_eq!(data.cpu.history, vec![2.3]);

        sample_cpu(&c, 2);
        c.clock.advance(2_000);
        // 1 to 2: busy +48 (user 37, system 11) out of +2392 total.
        assert_eq!(c.system.step().unwrap().1.cpu.history, vec![2.3, 2.0]);
    }

    #[test]
    fn cpu_history_keeps_last_sixty_seconds() {
        let mut c = notebook();
        c.system.show();
        let mut data = None;
        for n in 0..40 {
            c.root.write(
                "/proc/stat",
                &format!("cpu  {} 0 0 {} 0 0 0 0 0 0\n", n * 10, n * 90),
            );
            data = c.system.step();
            c.clock.advance(2_000);
        }

        assert_eq!(data.unwrap().1.cpu.history.len(), 30);
    }

    /// Open the module again if needed and read its temperature.
    fn temperature(c: &mut Scenario) -> Option<Temperature> {
        c.system.hide();
        c.system.show();
        c.system.step().unwrap().1.cpu.temperature
    }

    #[test]
    fn cpu_temperature_uses_k10temp_and_ignores_other_sensors() {
        let mut c = notebook();

        // k10temp Tctl 47375 millidegrees Celsius; ignore the neighboring NVMe sensor and its threshold.
        assert_eq!(
            temperature(&mut c),
            Some(Temperature {
                celsius: 47.4,
                warning: false
            })
        );
    }

    #[test]
    fn missing_cpu_sensor_hides_temperature() {
        let mut c = notebook();
        c.root.remove("/sys/class/hwmon/hwmon4");

        assert_eq!(temperature(&mut c), None);
    }

    #[test]
    fn missing_critical_threshold_warns_at_ninety_degrees() {
        let mut c = notebook();
        c.root
            .write("/sys/class/hwmon/hwmon4/temp1_input", "89900\n");
        assert!(!temperature(&mut c).unwrap().warning);

        c.root
            .write("/sys/class/hwmon/hwmon4/temp1_input", "90000\n");
        assert!(temperature(&mut c).unwrap().warning);
    }

    /// Intel coretemp layout: package with its own critical threshold and one sensor per core.
    fn with_coretemp(c: &Scenario, package: &str, critical: &str) {
        c.root.remove("/sys/class/hwmon/hwmon4");
        for (file, value) in [
            ("name", "coretemp"),
            ("temp1_label", "Package id 0"),
            ("temp1_input", package),
            ("temp1_crit", critical),
            ("temp2_label", "Core 0"),
            ("temp2_input", "61000"),
            ("temp2_crit", "100000"),
        ] {
            c.root.write(
                &format!("/sys/class/hwmon/hwmon6/{file}"),
                &format!("{value}\n"),
            );
        }
    }

    #[test]
    fn coretemp_uses_package_sensor() {
        let mut c = notebook();
        with_coretemp(&c, "85000", "100000");

        assert_eq!(
            temperature(&mut c),
            Some(Temperature {
                celsius: 85.0,
                warning: false
            })
        );
    }

    #[test]
    fn ninety_degrees_warns_even_with_higher_critical_threshold() {
        let mut c = notebook();
        with_coretemp(&c, "89900", "100000");
        assert!(!temperature(&mut c).unwrap().warning);

        with_coretemp(&c, "90000", "100000");
        assert!(temperature(&mut c).unwrap().warning);
    }

    #[test]
    fn sensor_critical_threshold_below_ninety_degrees_warns() {
        let mut c = notebook();
        with_coretemp(&c, "79900", "80000");
        assert!(!temperature(&mut c).unwrap().warning);

        with_coretemp(&c, "80000", "80000");
        assert!(temperature(&mut c).unwrap().warning);
    }

    fn disks(c: &mut Scenario) -> Vec<Disk> {
        c.system.show();
        c.system.step().unwrap().1.disks
    }

    #[test]
    fn only_root_disk_is_physical_in_recording() {
        let mut c = notebook();

        // The fixture contains tmpfs, 30+ snap squashfs mounts, efivarfs, /boot/efi and pseudo filesystems.
        assert_eq!(
            disks(&mut c),
            vec![Disk {
                mount: "/".into(),
                // RootFilesystem statvfs: frsize 4096, blocks 38426996, free 19959216, available 17989040.
                used: (38_426_996 - 19_959_216) * 4096,
                total: 38_426_996 * 4096,
                free: 17_989_040 * 4096,
                warning: false,
            }]
        );
    }

    fn mount_filesystems(c: &Scenario, lines: &str) {
        let mountinfo = c.root.read("/proc/self/mountinfo").unwrap();
        c.root
            .write("/proc/self/mountinfo", &format!("{mountinfo}{lines}"));
    }

    fn space(total: u64, available: u64) -> Space {
        Space {
            total,
            free: available,
            available,
        }
    }

    #[test]
    fn one_disk_per_device_without_overlay_or_boot() {
        let mut c = notebook();
        mount_filesystems(
            &c,
            "900 29 8:17 / /mnt/data\\040external rw,relatime shared:500 - ext4 /dev/sdb1 rw\n\
             901 29 0:45 /@home /home rw,relatime shared:1 - btrfs /dev/sda1 rw\n\
             902 29 8:1 / /boot rw,relatime shared:501 - ext4 /dev/sda1 rw\n\
             903 29 0:90 / /var/lib/docker/overlay2/x/merged rw - overlay overlay rw\n",
        );
        c.root.set_space("/mnt/data external", space(1000, 400));
        c.root.set_space("/boot", space(1000, 400));
        c.root.set_space("/home", space(1000, 400));

        let points: Vec<String> = disks(&mut c).into_iter().map(|d| d.mount).collect();

        // /home is another subvolume (a different 0:NN) on the root partition; do not add another bars.
        assert_eq!(points, vec!["/", "/mnt/data external"]);
    }

    #[test]
    fn disk_usage_above_ninety_percent_warns() {
        let mut c = notebook();
        mount_filesystems(
            &c,
            "900 29 8:17 / /mnt/a rw - ext4 /dev/sdb1 rw\n\
             901 29 8:33 / /mnt/b rw - ext4 /dev/sdc1 rw\n",
        );
        c.root.set_space("/mnt/a", space(1000, 100));
        c.root.set_space("/mnt/b", space(1000, 99));

        let d = disks(&mut c);

        assert!(!d[1].warning, "90% usage is not above 90%");
        assert!(d[2].warning);
    }

    #[test]
    fn disks_are_read_every_thirty_seconds() {
        let mut c = notebook();
        c.system.show();
        let before = c.system.step().unwrap().1.disks;
        c.root.set_space("/", space(1000, 10));

        c.clock.advance(28_000);
        assert_eq!(c.system.step().unwrap().1.disks, before);

        c.clock.advance(2_000);
        assert_eq!(c.system.step().unwrap().1.disks[0].free, 10);
    }

    fn battery(c: &mut Scenario) -> Option<Battery> {
        c.system.show();
        c.system.step().unwrap().1.battery
    }

    #[test]
    fn plugged_in_notebook_shows_full_battery() {
        let mut c = notebook();

        assert_eq!(
            battery(&mut c),
            Some(Battery {
                percentage: 100.0,
                state: BatteryState::Full,
                seconds_remaining: None,
                warning: false,
            })
        );
    }

    #[test]
    fn discharging_battery_below_fifteen_percent_warns() {
        let mut c = notebook();
        c.upower.respond("notebook-discharging");

        assert_eq!(
            battery(&mut c),
            Some(Battery {
                percentage: 12.0,
                state: BatteryState::Discharging,
                seconds_remaining: Some(1800),
                warning: true,
            })
        );
    }

    #[test]
    fn desktop_without_battery_hides_block() {
        let mut c = notebook();
        c.upower.respond("desktop");

        assert_eq!(battery(&mut c), None);
    }

    #[test]
    fn missing_upower_hides_battery_and_preserves_other_data() {
        let mut c = notebook();
        c.upower.disconnect();

        assert_eq!(battery(&mut c), None);
    }

    #[test]
    fn battery_is_read_only_on_upower_signal() {
        let mut c = notebook();
        c.system.show();
        sample_cpu(&c, 0);
        c.system.step().unwrap();
        c.upower.respond("notebook-discharging");

        sample_cpu(&c, 1);
        c.clock.advance(2_000);
        let (_, data) = c.system.step().unwrap();
        assert_eq!(data.battery.unwrap().state, BatteryState::Full);

        c.clock.advance(500);
        let (when, data) = c.system.battery_changed().unwrap();
        assert_eq!(when, 1_002_500);
        assert_eq!(data.battery.unwrap().state, BatteryState::Discharging);
        assert_eq!(
            data.cpu.history.len(),
            1,
            "the remainder of the snapshot is preserved"
        );
    }

    fn upower_read_count(c: &Scenario) -> usize {
        c.upower.readings.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[test]
    fn hidden_module_collects_nothing() {
        let mut c = notebook();

        assert_eq!(c.system.deadline(), None);
        assert_eq!(c.system.step(), None);
        assert_eq!(c.system.battery_changed(), None);
        assert_eq!(upower_read_count(&c), 0);
    }

    #[test]
    fn hiding_module_stops_collection() {
        let mut c = notebook();
        c.system.show();
        c.system.step().unwrap();
        let read_count = upower_read_count(&c);

        c.system.hide();
        c.clock.advance(60_000);

        assert_eq!(c.system.deadline(), None);
        assert_eq!(c.system.step(), None);
        assert_eq!(c.system.battery_changed(), None);
        assert_eq!(upower_read_count(&c), read_count);
    }

    #[test]
    fn reopening_module_restarts_empty_history() {
        let mut c = notebook();
        c.system.show();
        for n in 0..3 {
            sample_cpu(&c, n);
            c.system.step().unwrap();
            c.clock.advance(2_000);
        }
        c.system.hide();
        c.clock.advance(60_000);

        c.system.show();
        let cpu = c.system.step().unwrap().1.cpu;

        assert_eq!(cpu.usage, None);
        assert!(cpu.history.is_empty());
    }

    #[test]
    fn repeated_visibility_notice_preserves_history() {
        let mut c = notebook();
        c.system.show();
        sample_cpu(&c, 0);
        c.system.step().unwrap();
        sample_cpu(&c, 1);
        c.clock.advance(2_000);
        c.system.step().unwrap();

        c.system.show();

        assert_eq!(c.system.deadline(), Some(1_004_000));
        assert_eq!(c.system.battery_changed().unwrap().1.cpu.history, vec![2.3]);
    }

    /// UI snapshot scenarios in src/lib/system/fixtures/.
    fn ui_scenario(name: &str) -> (u64, SystemData) {
        let mut c = notebook();
        // 2026-10-30 14:32 UTC.
        c.clock.advance(1_793_457_120_000 - 1_000_000);
        match name {
            "notebook" => {}
            "desktop" => c.upower.respond("desktop"),
            "no-sensor" => c.root.remove("/sys/class/hwmon/hwmon4"),
            "warning" => {
                c.root
                    .write("/sys/class/hwmon/hwmon4/temp1_input", "92125\n");
                c.upower.respond("notebook-discharging");
                mount_filesystems(&c, "900 29 8:17 / /mnt/data rw - ext4 /dev/sdb1 rw\n");
                c.root
                    .set_space("/mnt/data", space(1_000_000_000_000, 42_000_000_000));
            }
            "missing" => {
                c.root.remove("/sys/class/hwmon/hwmon4");
                c.upower.respond("desktop");
                let meminfo = c.root.read("/proc/meminfo").unwrap();
                c.root.write(
                    "/proc/meminfo",
                    &meminfo
                        .replace("SwapTotal:       4194300 kB", "SwapTotal:             0 kB")
                        .replace("SwapFree:        3801600 kB", "SwapFree:              0 kB"),
                );
            }
            // Just opened: a single CPU sample, so usage is still unknown.
            "starting" => {
                c.system.show();
                sample_cpu(&c, 0);
                return c.system.step().unwrap();
            }
            _ => unreachable!(),
        }
        c.system.show();
        let mut last = None;
        for n in 0..3 {
            sample_cpu(&c, n);
            last = c.system.step();
            c.clock.advance(2_000);
        }
        last.unwrap()
    }

    /// The UI uses snapshots actually published by the core. Regenerate after format changes with
    /// `UPDATE_FIXTURES=1 cargo test`.
    #[test]
    fn ui_snapshot_fixtures_come_from_core() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/lib/system/fixtures");
        for name in [
            "notebook",
            "desktop",
            "no-sensor",
            "warning",
            "missing",
            "starting",
        ] {
            let (when, data) = ui_scenario(name);
            let json = serde_json::to_string_pretty(&snapshot(when, &data)).unwrap() + "\n";
            let file = dir.join(format!("{name}.json"));
            if std::env::var_os("UPDATE_FIXTURES").is_some() {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(&file, json).unwrap();
            } else {
                let recorded = std::fs::read_to_string(&file).unwrap_or_default();
                assert_eq!(
                    recorded, json,
                    "{name}.json is stale: run with UPDATE_FIXTURES=1"
                );
            }
        }
    }
}
