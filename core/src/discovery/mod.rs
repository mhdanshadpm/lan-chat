//! Peer discovery.
//!
//! Two independent mechanisms run at the same time:
//!
//! 1. mDNS/DNS-SD (`_lanchat._tcp.local.`) using `mdns-sd`.
//! 2. A UDP broadcast beacon on a fixed port (fallback for networks that
//!    filter multicast, and for platforms with weak mDNS support).
//!
//! The beacon also carries a `disc_port` so a second instance on the same
//! machine (which cannot bind the fixed port) can still be reached via
//! unicast replies.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::identity::Identity;
use crate::protocol::{PROTOCOL_VERSION, SERVICE_TYPE};

const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Announcement {
    pub v: u16,
    pub peer_id: Uuid,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub avatar_color: u32,
    #[serde(default)]
    pub device_name: String,
    /// WebSocket port of the announcing instance.
    pub port: u16,
    /// UDP port the announcer is listening on (may differ from the default).
    #[serde(default)]
    pub disc_port: u16,
    #[serde(default)]
    pub secure: bool,
    pub sent_at: i64,
}

impl Announcement {
    pub fn new(me: &Identity, ws_port: u16, disc_port: u16, secure: bool) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            peer_id: me.peer_id,
            display_name: me.display_name.clone(),
            avatar_color: me.avatar_color,
            device_name: me.device_name.clone(),
            port: ws_port,
            disc_port,
            secure,
            sent_at: crate::now_ms(),
        }
    }

    fn validate(&self) -> std::result::Result<(), String> {
        if self.v != PROTOCOL_VERSION {
            return Err(format!("unsupported discovery version {}", self.v));
        }
        if self.port == 0 {
            return Err("missing WebSocket port".into());
        }
        if self.display_name.chars().count() > 64 {
            return Err("display name too long".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum DiscoveryEvent {
    Found {
        announcement: Announcement,
        addr: IpAddr,
    },
    Lost(Uuid),
}

pub struct Discovery {
    stop: Arc<AtomicBool>,
    daemon: Option<ServiceDaemon>,
    udp_task: Option<tokio::task::JoinHandle<()>>,
    mdns_thread: Option<std::thread::JoinHandle<()>>,
    disc_port: u16,
    fullname: String,
}

impl Discovery {
    pub fn start(
        me: &Identity,
        ws_port: u16,
        secure: bool,
        disc_port: u16,
        tx: broadcast::Sender<DiscoveryEvent>,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let announcement = Arc::new(Announcement::new(me, ws_port, disc_port, secure));

        let (daemon, fullname) = match start_mdns(me, ws_port, secure, disc_port, tx.clone()) {
            Ok(value) => (Some(value.0), value.1),
            Err(err) => {
                tracing::warn!(?err, "mDNS unavailable, relying on UDP broadcast");
                (None, String::new())
            }
        };

        let mdns_thread = daemon.as_ref().and_then(|_| {
            let tx = tx.clone();
            let stop = stop.clone();
            let daemon = daemon.clone();
            let me_id = me.peer_id;
            let fullname = fullname.clone();
            std::thread::Builder::new()
                .name("lanchat-mdns".into())
                .spawn(move || {
                    let Some(daemon) = daemon else { return };
                    let receiver = match daemon.browse(SERVICE_TYPE) {
                        Ok(receiver) => receiver,
                        Err(err) => {
                            tracing::warn!(?err, "mDNS browse failed");
                            return;
                        }
                    };
                    loop {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        match receiver.recv_timeout(Duration::from_millis(500)) {
                            Ok(ServiceEvent::ServiceResolved(info)) => {
                                if info.get_fullname() == fullname {
                                    continue;
                                }
                                let peer_id = info
                                    .get_property_val_str("peer_id")
                                    .and_then(|s| Uuid::parse_str(s).ok())
                                    .or_else(|| {
                                        info.get_fullname()
                                            .split('.')
                                            .next()
                                            .and_then(|s| Uuid::parse_str(s).ok())
                                    });
                                let Some(peer_id) = peer_id else { continue };
                                if peer_id == me_id {
                                    continue;
                                }
                                let addr = info
                                    .get_addresses()
                                    .iter()
                                    .find(|ip| ip.is_ipv4())
                                    .or_else(|| info.get_addresses().iter().next())
                                    .copied();
                                let Some(addr) = addr else { continue };
                                let announcement = Announcement {
                                    v: PROTOCOL_VERSION,
                                    peer_id,
                                    display_name: info
                                        .get_property_val_str("name")
                                        .unwrap_or_default()
                                        .to_string(),
                                    avatar_color: info
                                        .get_property_val_str("avatar")
                                        .and_then(|s| s.parse::<u32>().ok())
                                        .unwrap_or(0),
                                    device_name: info
                                        .get_property_val_str("device")
                                        .unwrap_or_default()
                                        .to_string(),
                                    port: info.get_port(),
                                    disc_port: info
                                        .get_property_val_str("disc_port")
                                        .and_then(|s| s.parse::<u16>().ok())
                                        .unwrap_or(crate::protocol::DEFAULT_DISCOVERY_PORT),
                                    secure: info
                                        .get_property_val_str("secure")
                                        .map(|s| s == "1")
                                        .unwrap_or(false),
                                    sent_at: crate::now_ms(),
                                };
                                let _ = tx.send(DiscoveryEvent::Found { announcement, addr });
                            }
                            Ok(ServiceEvent::ServiceRemoved(_ty, fullname)) => {
                                if let Some(instance) = fullname.split('.').next() {
                                    if let Ok(id) = Uuid::parse_str(instance) {
                                        let _ = tx.send(DiscoveryEvent::Lost(id));
                                    }
                                }
                            }
                            Ok(_) => {}
                            Err(_) => continue,
                        }
                    }
                    let _ = daemon.shutdown();
                })
                .ok()
        });

        let udp_task = tokio::spawn(udp_beacon(announcement, tx, stop.clone()));

        Ok(Self {
            stop,
            daemon,
            udp_task: Some(udp_task),
            mdns_thread,
            disc_port,
            fullname,
        })
    }

    pub fn disc_port(&self) -> u16 {
        self.disc_port
    }

    pub fn mdns_active(&self) -> bool {
        self.daemon.is_some()
    }

    pub fn fullname(&self) -> &str {
        &self.fullname
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(task) = self.udp_task.take() {
            task.abort();
        }
        if let Some(daemon) = self.daemon.take() {
            let _ = daemon.stop_browse(SERVICE_TYPE);
            let _ = daemon.shutdown();
        }
        if let Some(thread) = self.mdns_thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.stop();
    }
}

fn start_mdns(
    me: &Identity,
    ws_port: u16,
    secure: bool,
    disc_port: u16,
    _tx: broadcast::Sender<DiscoveryEvent>,
) -> Result<(ServiceDaemon, String)> {
    let daemon = ServiceDaemon::new()
        .map_err(|e| CoreError::Discovery(format!("mDNS daemon: {e}")))?;
    let instance = me.peer_id.to_string();
    let host_name = format!(
        "lanchat-{}.local.",
        &me.peer_id.to_string()[..8]
    );
    let ip = local_ipv4().map(|ip| ip.to_string()).unwrap_or_default();
    let name: String = me.display_name.chars().take(60).collect();
    let device: String = me.device_name.chars().take(60).collect();
    let properties = [
        ("peer_id", instance.clone()),
        ("name", if name.is_empty() { "LAN Chat".to_string() } else { name }),
        ("avatar", me.avatar_color.to_string()),
        ("device", device),
        ("proto", PROTOCOL_VERSION.to_string()),
        ("secure", if secure { "1".to_string() } else { "0".to_string() }),
        ("disc_port", disc_port.to_string()),
    ];
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &host_name,
        ip.as_str(),
        ws_port,
        &properties[..],
    )
    .map_err(|e| CoreError::Discovery(format!("mDNS service info: {e}")))?;
    let fullname = info.get_fullname().to_string();
    daemon
        .register(info)
        .map_err(|e| CoreError::Discovery(format!("mDNS register: {e}")))?;
    Ok((daemon, fullname))
}

async fn udp_beacon(
    announcement: Arc<Announcement>,
    tx: broadcast::Sender<DiscoveryEvent>,
    stop: Arc<AtomicBool>,
) {
    let preferred = announcement.disc_port;
    let socket = match bind_udp(preferred) {
        Ok(socket) => socket,
        Err(err) => {
            tracing::warn!(?err, port = preferred, "UDP discovery bind failed");
            return;
        }
    };
    let local_port = socket.local_addr().map(|a| a.port()).unwrap_or(preferred);
    if local_port != preferred {
        tracing::info!(local_port, "UDP discovery using fallback ephemeral port");
    }
    let mut announcement = (*announcement).clone();
    announcement.disc_port = local_port;
    let payload = Arc::new(match serde_json::to_vec(&announcement) {
        Ok(payload) => payload,
        Err(err) => {
            tracing::error!(?err, "cannot serialise discovery beacon");
            return;
        }
    });

    let mut buf = vec![0u8; 2048];
    let mut interval = tokio::time::interval(ANNOUNCE_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_error_log = None::<std::time::Instant>;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        tokio::select! {
            _ = interval.tick() => {
                for target in beacon_targets(preferred) {
                    if let Err(err) = socket.send_to(&payload, target).await {
                        let now = std::time::Instant::now();
                        if last_error_log.map(|t| now.duration_since(t).as_secs() > 30).unwrap_or(true) {
                            tracing::debug!(?err, %target, "discovery broadcast failed");
                            last_error_log = Some(now);
                        }
                    }
                }
            }
            received = socket.recv_from(&mut buf) => {
                let Ok((len, src)) = received else { continue };
                let Ok(parsed) = serde_json::from_slice::<Announcement>(&buf[..len]) else { continue };
                if parsed.peer_id == announcement.peer_id {
                    continue;
                }
                if let Err(reason) = parsed.validate() {
                    tracing::debug!(%reason, "ignoring invalid beacon");
                    continue;
                }
                let _ = tx.send(DiscoveryEvent::Found {
                    announcement: parsed.clone(),
                    addr: src.ip(),
                });
                // A peer with a non-default port cannot hear broadcasts sent to
                // the default port, so answer it directly.
                if parsed.disc_port != 0 && parsed.disc_port != local_port {
                    let _ = socket
                        .send_to(&payload, SocketAddr::new(src.ip(), parsed.disc_port))
                        .await;
                }
            }
        }
    }
}

fn bind_udp(port: u16) -> Result<UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_reuse_address(true)?;
    socket.set_broadcast(true)?;
    let addr: SocketAddr = SocketAddr::from(([0, 0, 0, 0], port));
    socket.bind(&addr.into())?;
    socket.set_nonblocking(true)?;
    let std_socket: std::net::UdpSocket = socket.into();
    Ok(UdpSocket::from_std(std_socket)?)
}

fn beacon_targets(port: u16) -> Vec<SocketAddr> {
    let mut targets = vec![SocketAddr::new(
        IpAddr::V4(Ipv4Addr::BROADCAST),
        port,
    )];
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            if iface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                let broadcast = v4.broadcast.unwrap_or_else(|| {
                    let ip = u32::from(v4.ip);
                    let mask = u32::from(v4.netmask);
                    Ipv4Addr::from(ip | !mask)
                });
                let target = SocketAddr::new(IpAddr::V4(broadcast), port);
                if !targets.contains(&target) {
                    targets.push(target);
                }
            }
        }
    }
    targets
}

pub fn local_ipv4() -> Option<Ipv4Addr> {
    let interfaces = if_addrs::get_if_addrs().ok()?;
    let mut fallback = None;
    for iface in interfaces {
        if iface.is_loopback() {
            continue;
        }
        if let if_addrs::IfAddr::V4(v4) = iface.addr {
            if v4.ip.is_link_local() {
                fallback.get_or_insert(v4.ip);
                continue;
            }
            return Some(v4.ip);
        }
    }
    fallback
}

/// A cheap fingerprint of the machine's IPv4 addresses, used to detect Wi-Fi
/// switches / cable changes.
pub fn network_fingerprint() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        let mut addrs: Vec<String> = interfaces
            .into_iter()
            .filter(|i| !i.is_loopback())
            .map(|i| i.ip().to_string())
            .collect();
        addrs.sort();
        addrs.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announcement_roundtrip() {
        let me = Identity::new("Alice", Some(0x112233));
        let ann = Announcement::new(&me, 43210, 47823, true);
        assert!(ann.validate().is_ok());
        let raw = serde_json::to_string(&ann).unwrap();
        let parsed: Announcement = serde_json::from_str(&raw).unwrap();
        assert_eq!(ann, parsed);
        assert_eq!(parsed.peer_id, me.peer_id);
    }

    #[test]
    fn announcement_rejects_wrong_version() {
        let me = Identity::new("Alice", None);
        let mut ann = Announcement::new(&me, 43210, 47823, false);
        ann.v = 99;
        assert!(ann.validate().is_err());
    }

    #[test]
    fn announcement_rejects_missing_port() {
        let me = Identity::new("Alice", None);
        let ann = Announcement::new(&me, 0, 47823, false);
        assert!(ann.validate().is_err());
    }

    #[test]
    fn beacon_targets_include_limited_broadcast() {
        let targets = beacon_targets(47823);
        assert!(targets.contains(&SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), 47823)));
        assert!(targets.len() >= 1);
    }

    #[test]
    fn network_fingerprint_is_stable_between_calls() {
        assert_eq!(network_fingerprint(), network_fingerprint());
    }
}
