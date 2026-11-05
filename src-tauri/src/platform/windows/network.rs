use std::{
    hash::{BuildHasher, RandomState},
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    ptr,
    sync::LazyLock,
};
use windows::{
    core::Interface,
    Win32::{
        Foundation::{ERROR_SUCCESS, HANDLE},
        NetworkManagement::{IpHelper::*, Ndis::IfOperStatusUp},
        Networking::{NetworkListManager::*, WinSock::*},
        System::Com::*,
    },
};

use crate::{
    network::{Connection, Counters, NetworkManager, Probe, INTERNET_V4, INTERNET_V6},
    ports::Files,
};

pub(super) fn with_network_manager<T>(
    query: impl FnOnce(INetworkListManager) -> windows::core::Result<T>,
) -> io::Result<T> {
    let unavailable = |_| io::Error::other("connectivity unavailable");
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }.map_err(unavailable)?;
    let result = unsafe { CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL) }.and_then(query);
    unsafe { CoUninitialize() };
    result.map_err(unavailable)
}

pub struct WindowsNetwork;

impl NetworkManager for WindowsNetwork {
    fn connection(&self) -> io::Result<Connection> {
        let (connected, online, metered) = with_network_manager(|manager| {
            let connected = unsafe { manager.IsConnected() }?.as_bool();
            let online = unsafe { manager.IsConnectedToInternet() }?.as_bool();
            let mut cost = 0;
            let unrestricted = manager.cast::<INetworkCostManager>().is_ok_and(|manager| {
                unsafe { manager.GetCost(&mut cost, ptr::null()) }.is_ok()
                    && cost == NLM_CONNECTION_COST_UNRESTRICTED.0 as u32
            });
            Ok((connected || online, online, !unrestricted))
        })?;
        if !connected {
            return Ok(Connection::default());
        }
        let route = route(INTERNET_V4.into()).or_else(|| route(INTERNET_V6.into()));
        let mut connection = Connection {
            id: "connected".into(),
            connectivity: if online { 4 } else { 1 },
            // An unknown or restricted cost is not a safe default for an automatic transfer.
            metered: metered || route.is_none(),
            ..Default::default()
        };
        if let Some((route, source, interface)) = route {
            let gateway = address(route.NextHop).filter(|ip| !ip.is_unspecified());
            connection.id = unsafe { interface.InterfaceLuid.Value }.to_string();
            connection.route_id = route_id(source, gateway);
            connection.addresses = vec![source];
            connection.interface = interface.InterfaceIndex.to_string();
            connection.gateway = gateway;
            // IANA interface types as returned by MIB_IF_ROW2.
            connection.kind = match interface.Type {
                6 => "802-3-ethernet",
                71 => "802-11-wireless",
                _ => "other",
            }
            .into();
            connection.vpn = interface.Type == 131;
        }
        Ok(connection)
    }

    fn counters(&self, _: &dyn Files, connection: &Connection) -> io::Result<Counters> {
        let row = connection
            .interface
            .parse()
            .ok()
            .and_then(interface)
            .filter(|row| unsafe { row.InterfaceLuid.Value }.to_string() == connection.id)
            .ok_or_else(|| io::Error::other("traffic unavailable"))?;
        Ok(Counters::from([(
            connection.interface.clone(),
            (row.InOctets, row.OutOctets),
        )]))
    }

    fn interface_index(&self, _: &dyn Files, interface: &str) -> u32 {
        interface.parse().unwrap_or(0)
    }

    fn advanced_available(&self) -> bool {
        false
    }
}

fn route_id(source: IpAddr, gateway: Option<IpAddr>) -> String {
    // The speed cache keeps an opaque route fingerprint. Addresses stay in memory.
    static KEY: LazyLock<RandomState> = LazyLock::new(RandomState::new);
    format!("{:016x}", KEY.hash_one((source, gateway)))
}

fn interface(index: u32) -> Option<MIB_IF_ROW2> {
    let mut row = MIB_IF_ROW2 {
        InterfaceIndex: index,
        ..Default::default()
    };
    (unsafe { GetIfEntry2(&mut row) } == ERROR_SUCCESS && row.OperStatus == IfOperStatusUp)
        .then_some(row)
}

fn route(ip: IpAddr) -> Option<(MIB_IPFORWARD_ROW2, IpAddr, MIB_IF_ROW2)> {
    let destination = match ip {
        IpAddr::V4(ip) => SOCKADDR_INET {
            Ipv4: SOCKADDR_IN {
                sin_family: AF_INET,
                sin_addr: IN_ADDR {
                    S_un: IN_ADDR_0 {
                        S_addr: u32::from_ne_bytes(ip.octets()),
                    },
                },
                ..Default::default()
            },
        },
        IpAddr::V6(ip) => SOCKADDR_INET { Ipv6: ipv6(ip, 0) },
    };
    let mut index = 0;
    if unsafe { GetBestInterfaceEx(ptr::from_ref(&destination).cast(), &mut index) } != 0 {
        return None;
    }
    let mut route = MIB_IPFORWARD_ROW2::default();
    let mut source = SOCKADDR_INET::default();
    if unsafe { GetBestRoute2(None, index, None, &destination, 0, &mut route, &mut source) }
        != ERROR_SUCCESS
    {
        return None;
    }
    Some((route, address(source)?, interface(index)?))
}

fn address(address: SOCKADDR_INET) -> Option<IpAddr> {
    match unsafe { address.si_family } {
        AF_INET => {
            Some(Ipv4Addr::from(unsafe { address.Ipv4.sin_addr.S_un.S_addr }.to_ne_bytes()).into())
        }
        AF_INET6 => Some(Ipv6Addr::from(unsafe { address.Ipv6.sin6_addr.u.Byte }).into()),
        _ => None,
    }
}

fn ipv6(ip: Ipv6Addr, scope: u32) -> SOCKADDR_IN6 {
    SOCKADDR_IN6 {
        sin6_family: AF_INET6,
        sin6_addr: IN6_ADDR {
            u: IN6_ADDR_0 { Byte: ip.octets() },
        },
        Anonymous: SOCKADDR_IN6_0 {
            sin6_scope_id: scope,
        },
        ..Default::default()
    }
}

pub struct WindowsPing;

struct Icmp(HANDLE);
impl Drop for Icmp {
    fn drop(&mut self) {
        let _ = unsafe { IcmpCloseHandle(self.0) };
    }
}

impl Probe for WindowsPing {
    fn measure(&self, destination: SocketAddr) -> Option<f64> {
        const TIMEOUT_MS: u32 = 700;
        let payload = b"kvojps01";
        // A reply header followed by payload and ICMP error space. The u64 array keeps it aligned.
        let mut buffer = [0u64; 32];
        let reply = buffer.as_mut_ptr().cast();
        let size = std::mem::size_of_val(&buffer) as u32;
        match destination {
            SocketAddr::V4(destination) => {
                let handle = Icmp(unsafe { IcmpCreateFile() }.ok()?);
                let count = unsafe {
                    IcmpSendEcho(
                        handle.0,
                        u32::from_ne_bytes(destination.ip().octets()),
                        payload.as_ptr().cast(),
                        payload.len() as u16,
                        None,
                        reply,
                        size,
                        TIMEOUT_MS,
                    )
                };
                if count == 0 {
                    return None;
                }
                let reply = unsafe { &*reply.cast::<ICMP_ECHO_REPLY>() };
                (reply.Status == 0).then_some(f64::from(reply.RoundTripTime))
            }
            SocketAddr::V6(destination) => {
                let handle = Icmp(unsafe { Icmp6CreateFile() }.ok()?);
                let source = ipv6(Ipv6Addr::UNSPECIFIED, 0);
                let destination = ipv6(*destination.ip(), destination.scope_id());
                let count = unsafe {
                    Icmp6SendEcho2(
                        handle.0,
                        None,
                        None,
                        None,
                        &source,
                        &destination,
                        payload.as_ptr().cast(),
                        payload.len() as u16,
                        None,
                        reply,
                        size,
                        TIMEOUT_MS,
                    )
                };
                if count == 0 {
                    return None;
                }
                let reply = unsafe { &*reply.cast::<ICMPV6_ECHO_REPLY_LH>() };
                (reply.Status == 0).then_some(f64::from(reply.RoundTripTime))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv6_scope_does_not_change_address_bytes() {
        let ip = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1);
        let scoped = ipv6(ip, 7);
        assert_eq!(unsafe { scoped.Anonymous.sin6_scope_id }, 7);
        assert_eq!(address(SOCKADDR_INET { Ipv6: scoped }), Some(ip.into()));
    }

    #[test]
    fn native_icmp_can_measure_loopback_without_external_network() {
        assert!(WindowsPing
            .measure(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0))
            .is_some());
    }

    #[test]
    fn native_route_provides_one_interface_for_traffic_when_connected() {
        let source = WindowsNetwork;
        let connection = source.connection().unwrap();
        if connection.connectivity != 4 {
            return;
        }
        assert!(!connection.interface.is_empty());
        let counters = source
            .counters(&crate::ports::RootFilesystem::system(), &connection)
            .unwrap();
        assert_eq!(counters.len(), 1);
        assert!(counters.contains_key(&connection.interface));
    }
}
