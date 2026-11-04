//! TCP attribution is ephemeral: socket addresses, owners and counters never reach disk.
use super::{Counters, Traffic};
use crate::ports::Files;
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    io,
};

/// Kernel I/O boundary. Dumps contain native-endian netlink messages for IPv4 and IPv6.
pub trait Diagnostics: Send {
    fn uid(&self) -> u32;
    fn dump(&self) -> io::Result<Vec<u8>>;
}
#[derive(Clone, Debug, Serialize)]
pub struct AppTraffic {
    pub name: String,
    pub download: f64,
    pub upload: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProcessTraffic {
    pub apps: Vec<AppTraffic>,
    pub others: Traffic,
}
#[derive(Clone)]
struct SocketCounter {
    inode: u32,
    bytes: ByteCounters,
}
#[derive(Clone, Copy, Default)]
struct ByteCounters {
    received: u64,
    sent: u64,
}
impl ByteCounters {
    fn add(&mut self, other: Self) {
        self.received = self.received.saturating_add(other.received);
        self.sent = self.sent.saturating_add(other.sent);
    }
    fn since(self, previous: Self) -> Option<Self> {
        Some(Self {
            received: self.received.checked_sub(previous.received)?,
            sent: self.sent.checked_sub(previous.sent)?,
        })
    }
}
type Sockets = HashMap<u64, SocketCounter>;
struct Sample {
    start: u64,
    end: u64,
    apps: HashMap<String, ByteCounters>,
    total: ByteCounters,
}
pub struct Attribution {
    diagnostics: Box<dyn Diagnostics>,
    visible: bool,
    next: u64,
    previous: Option<(u64, Sockets, ByteCounters)>,
    samples: VecDeque<Sample>,
    pub data: Option<ProcessTraffic>,
}
impl Attribution {
    pub fn new(diagnostics: impl Diagnostics + 'static) -> Self {
        Self {
            diagnostics: Box::new(diagnostics),
            visible: false,
            next: 0,
            previous: None,
            samples: VecDeque::new(),
            data: None,
        }
    }
    pub fn visible(&mut self, visible: bool) {
        if self.visible != visible {
            self.reset();
        }
        self.visible = visible;
    }
    pub fn reset(&mut self) {
        self.previous = None;
        self.samples.clear();
        self.data = None;
        self.next = 0;
    }
    pub fn step(
        &mut self,
        now: u64,
        files: &impl Files,
        interface: &str,
        addresses: &[std::net::IpAddr],
        counters: Option<&Counters>,
    ) {
        if !self.visible || now < self.next {
            return;
        }
        self.next = now.saturating_add(2000);
        let result = (|| {
            let &(received, sent) = counters?.get(interface)?;
            let total = ByteCounters { received, sent };
            let bytes = self.diagnostics.dump().ok()?;
            let sockets = decode(&bytes, self.diagnostics.uid(), addresses).ok()?;
            Some((sockets, total))
        })();
        let Some((sockets, total)) = result else {
            self.reset();
            self.next = now.saturating_add(2000);
            return;
        };
        let previous = self.previous.replace((now, sockets.clone(), total));
        let Some((when, previous, old_total)) = previous else {
            return;
        };
        let delta = total.since(old_total);
        if now <= when || delta.is_none() {
            self.samples.clear();
            self.data = None;
            return;
        }
        let owners = owners(files, self.diagnostics.uid());
        let mut apps = HashMap::<String, ByteCounters>::new();
        for (cookie, socket) in sockets {
            let Some(old) = previous.get(&cookie).filter(|s| s.inode == socket.inode) else {
                continue;
            };
            let Some(name) = owners.get(&socket.inode) else {
                continue;
            };
            let Some(delta) = socket.bytes.since(old.bytes) else {
                continue;
            };
            apps.entry(name.clone()).or_default().add(delta);
        }
        self.samples.push_back(Sample {
            start: when,
            end: now,
            apps,
            total: delta.unwrap(),
        });
        // Three intervals at the 2 s cadence give a ~5 s moving average (6 s).
        while self
            .samples
            .front()
            .is_some_and(|s| now.saturating_sub(s.end) >= 5000)
        {
            self.samples.pop_front();
        }
        let seconds = (now - self.samples.front().unwrap().start) as f64 / 1000.0;
        let mut grouped = HashMap::<String, ByteCounters>::new();
        let mut totals = ByteCounters::default();
        for sample in &self.samples {
            totals.add(sample.total);
            for (name, bytes) in &sample.apps {
                grouped.entry(name.clone()).or_default().add(*bytes);
            }
        }
        let mut apps: Vec<_> = grouped
            .into_iter()
            .filter(|(_, bytes)| bytes.received > 0 || bytes.sent > 0)
            .map(|(name, bytes)| AppTraffic {
                name,
                download: bytes.received as f64 / seconds,
                upload: bytes.sent as f64 / seconds,
            })
            .collect();
        let attributed_rx: f64 = apps.iter().map(|a| a.download).sum();
        let attributed_tx: f64 = apps.iter().map(|a| a.upload).sum();
        apps.sort_by(|a, b| {
            (b.download + b.upload)
                .total_cmp(&(a.download + a.upload))
                .then_with(|| a.name.cmp(&b.name))
        });
        apps.truncate(5);
        self.data = Some(ProcessTraffic {
            apps,
            others: Traffic {
                download: (totals.received as f64 / seconds - attributed_rx).max(0.0),
                upload: (totals.sent as f64 / seconds - attributed_tx).max(0.0),
            },
        });
    }
}
fn owners(files: &impl Files, uid: u32) -> HashMap<u32, String> {
    let mut owners = HashMap::new();
    let mut processes = files.list("/proc").unwrap_or_default();
    processes.sort();
    for pid in processes.into_iter().filter(|p| p.parse::<u32>().is_ok()) {
        let root = format!("/proc/{pid}");
        let Ok(status) = files.read(&format!("{root}/status")) else {
            continue;
        };
        let own = status
            .lines()
            .find_map(|l| l.strip_prefix("Uid:"))
            .is_some_and(|l| {
                l.split_whitespace()
                    .take(2)
                    .all(|v| v.parse::<u32>() == Ok(uid))
                    && l.split_whitespace().count() >= 2
            });
        if !own {
            continue;
        }
        let name = files
            .read_link(&format!("{root}/exe"))
            .ok()
            .and_then(|p| {
                p.file_name().map(|s| {
                    s.to_string_lossy()
                        .trim_end_matches(" (deleted)")
                        .to_string()
                })
            })
            .or_else(|| {
                files
                    .read(&format!("{root}/comm"))
                    .ok()
                    .map(|s| s.trim().to_string())
            });
        let Some(name) = name.filter(|n| !n.is_empty()) else {
            continue;
        };
        for fd in files.list(&format!("{root}/fd")).unwrap_or_default() {
            let Ok(link) = files.read_link(&format!("{root}/fd/{fd}")) else {
                continue;
            };
            if let Some(inode) = link
                .to_str()
                .and_then(|s| s.strip_prefix("socket:["))
                .and_then(|s| s.strip_suffix(']'))
                .and_then(|s| s.parse().ok())
            {
                owners.entry(inode).or_insert_with(|| name.clone());
            }
        }
    }
    owners
}
fn u32_at(bytes: &[u8], at: usize) -> io::Result<u32> {
    Ok(u32::from_ne_bytes(
        bytes
            .get(at..at + 4)
            .ok_or(io::ErrorKind::InvalidData)?
            .try_into()
            .unwrap(),
    ))
}
fn u64_at(bytes: &[u8], at: usize) -> io::Result<u64> {
    Ok(u64::from_ne_bytes(
        bytes
            .get(at..at + 8)
            .ok_or(io::ErrorKind::InvalidData)?
            .try_into()
            .unwrap(),
    ))
}
fn u16_at(bytes: &[u8], at: usize) -> io::Result<u16> {
    Ok(u16::from_ne_bytes(
        bytes
            .get(at..at + 2)
            .ok_or(io::ErrorKind::InvalidData)?
            .try_into()
            .unwrap(),
    ))
}
fn aligned(n: usize) -> usize {
    (n + 3) & !3
}
fn decode(mut bytes: &[u8], uid: u32, addresses: &[std::net::IpAddr]) -> io::Result<Sockets> {
    let mut sockets = HashMap::new();
    while !bytes.is_empty() {
        let len = u32_at(bytes, 0)? as usize;
        if len < 16 || len > bytes.len() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let message = &bytes[..len];
        if u16_at(message, 4)? == 20 {
            let body = &message[16..];
            if body.len() < 72 {
                return Err(io::ErrorKind::InvalidData.into());
            }
            let address = |at| -> Option<std::net::IpAddr> {
                match body[0] {
                    2 => Some(
                        std::net::Ipv4Addr::from(<[u8; 4]>::try_from(&body[at..at + 4]).ok()?)
                            .into(),
                    ),
                    10 => Some(
                        std::net::Ipv6Addr::from(<[u8; 16]>::try_from(&body[at..at + 16]).ok()?)
                            .to_canonical(),
                    ),
                    _ => None,
                }
            };
            let local = address(8);
            let remote = address(24);
            let external = local.is_some_and(|ip| {
                !ip.is_loopback() && addresses.iter().any(|allowed| allowed.to_canonical() == ip)
            }) && remote.is_some_and(|ip| !ip.is_loopback());
            if external && u32_at(body, 64)? == uid {
                let mut at = 72;
                while at + 4 <= body.len() {
                    let length = u16_at(body, at)? as usize;
                    if length < 4 || at + length > body.len() {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    // linux/tcp.h: bytes_received at 128, bytes_sent at 200.
                    if u16_at(body, at + 2)? & 0x3fff == 2 && length >= 212 {
                        let info = &body[at + 4..at + length];
                        sockets.insert(
                            u64_at(body, 44)?,
                            SocketCounter {
                                inode: u32_at(body, 68)?,
                                bytes: ByteCounters {
                                    received: u64_at(info, 128)?,
                                    sent: u64_at(info, 200)?,
                                },
                            },
                        );
                    }
                    at += aligned(length);
                }
            }
        } else if u16_at(message, 4)? == 2 {
            return Err(io::ErrorKind::Other.into());
        }
        bytes = bytes.get(aligned(len)..).unwrap_or_default();
    }
    Ok(sockets)
}

pub struct KernelDiagnostics;
#[cfg(target_os = "linux")]
impl Diagnostics for KernelDiagnostics {
    fn uid(&self) -> u32 {
        unsafe { libc::getuid() }
    }
    fn dump(&self) -> io::Result<Vec<u8>> {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        // OwnedFd closes on every error path. The socket is unprivileged and bounded by a timeout.
        let raw = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                libc::NETLINK_SOCK_DIAG,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let timeout = libc::timeval {
            tv_sec: 1,
            tv_usec: 0,
        };
        if unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &timeout as *const _ as *const _,
                std::mem::size_of_val(&timeout) as _,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut address: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        address.nl_family = libc::AF_NETLINK as _;
        let mut output = Vec::new();
        for (seq, family) in [(1u32, libc::AF_INET), (2u32, libc::AF_INET6)] {
            let mut request = [0u8; 72];
            request[..4].copy_from_slice(&72u32.to_ne_bytes());
            request[4..6].copy_from_slice(&20u16.to_ne_bytes());
            request[6..8].copy_from_slice(&0x301u16.to_ne_bytes());
            request[8..12].copy_from_slice(&seq.to_ne_bytes());
            request[16] = family as u8;
            request[17] = 6;
            request[18] = 2;
            request[20..24].copy_from_slice(&u32::MAX.to_ne_bytes());
            if unsafe {
                libc::sendto(
                    fd.as_raw_fd(),
                    request.as_ptr() as _,
                    request.len(),
                    0,
                    &address as *const _ as _,
                    std::mem::size_of_val(&address) as _,
                )
            } < 0
            {
                return Err(io::Error::last_os_error());
            }
            'dump: loop {
                let mut buffer = [0u8; 65536];
                let count = unsafe {
                    libc::recv(
                        fd.as_raw_fd(),
                        buffer.as_mut_ptr() as _,
                        buffer.len(),
                        libc::MSG_TRUNC,
                    )
                };
                if count < 0 {
                    return Err(io::Error::last_os_error());
                }
                if count == 0 || count as usize > buffer.len() {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                let mut bytes = &buffer[..count as usize];
                while !bytes.is_empty() {
                    let len = u32_at(bytes, 0)? as usize;
                    if len < 16
                        || len > bytes.len()
                        || u32_at(bytes, 8)? != seq
                        || u16_at(bytes, 6)? & 0x10 != 0
                    {
                        return Err(io::ErrorKind::InvalidData.into());
                    }
                    match u16_at(bytes, 4)? {
                        3 => {
                            if len >= 20 && u32_at(bytes, 16)? != 0 {
                                return Err(io::ErrorKind::Other.into());
                            }
                            break 'dump;
                        }
                        2 => return Err(io::ErrorKind::Other.into()),
                        _ => output.extend_from_slice(&bytes[..aligned(len).min(bytes.len())]),
                    }
                    if output.len() > 32 * 1024 * 1024 {
                        return Err(io::ErrorKind::OutOfMemory.into());
                    }
                    bytes = bytes.get(aligned(len)..).unwrap_or_default();
                }
            }
        }
        Ok(output)
    }
}
