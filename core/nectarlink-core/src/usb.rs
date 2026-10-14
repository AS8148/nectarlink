// SPDX-License-Identifier: MPL-2.0
//! USB cable transport and USB auto-pairing via ADB port forwarding.
//!
//! When a phone is plugged into a PC over USB with USB debugging enabled:
//! 1. The phone listens on loopback TCP `127.0.0.1:53319` ([`DEFAULT_USB_PORT`]).
//! 2. The PC's ADB watcher forwards a local ephemeral TCP port to the phone's
//!    TCP port (`adb forward tcp:0 tcp:53319`) and opens a TCP connection.
//! 3. Both sides exchange a [`UsbHello`] header (`NLQU` magic + length-prefixed
//!    JSON) to learn each other's [`DeviceId`] and display name before any QUIC
//!    packets flow.
//! 4. Each side binds a loopback UDP socket (`127.0.0.1:0`) and bridges UDP
//!    datagrams over the length-prefixed TCP stream to its local iroh QUIC
//!    endpoint, so QUIC sessions (control stream, file transfers, screen
//!    mirroring, audio, camera, clipboard) run unmodified over the USB cable
//!    even when Wi-Fi is off.
//! 5. If the two devices are not paired yet, the PC automatically initiates the
//!    nearby 6-digit SAS code ceremony over the USB tunnel; the phone accepts
//!    the ceremony on a verified USB loopback connection without requiring manual
//!    host mode, and both users must confirm the 6-digit code before any trust
//!    is stored.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

use nectarlink_protocol::DeviceId;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream, UdpSocket},
};
use tokio_util::sync::CancellationToken;

use crate::{Error, Result, node::Shared};

/// Default loopback TCP port the Android app listens on for the USB tunnel.
pub const DEFAULT_USB_PORT: u16 = 53319;

pub use nectarlink_adb::DEFAULT_ADB_ADDR;

const MAGIC: &[u8; 4] = b"NLQU";
const MAX_HELLO_BYTES: usize = 4096;
const HELLO_TIMEOUT: Duration = Duration::from_secs(3);
const ADB_RETRY_DELAY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsbHello {
    v: u8,
    id: String,
    name: String,
}

async fn write_hello(stream: &mut TcpStream, id: &DeviceId, name: &str) -> Result<()> {
    let hello = UsbHello { v: 1, id: id.to_string(), name: name.to_owned() };
    let payload = serde_json::to_vec(&hello).map_err(|e| Error::Internal(e.to_string()))?;
    if payload.is_empty() || payload.len() > MAX_HELLO_BYTES {
        return Err(Error::Protocol("invalid USB hello length".into()));
    }
    stream.write_all(MAGIC).await.map_err(|e| Error::Network(e.to_string()))?;
    stream
        .write_all(&(payload.len() as u16).to_be_bytes())
        .await
        .map_err(|e| Error::Network(e.to_string()))?;
    stream.write_all(&payload).await.map_err(|e| Error::Network(e.to_string()))?;
    stream.flush().await.map_err(|e| Error::Network(e.to_string()))?;
    Ok(())
}

async fn read_hello(stream: &mut TcpStream) -> Result<(DeviceId, String)> {
    let mut magic = [0u8; 4];
    stream.read_exact(&mut magic).await.map_err(|e| Error::Network(e.to_string()))?;
    if &magic != MAGIC {
        return Err(Error::Protocol("invalid USB tunnel magic".into()));
    }
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf).await.map_err(|e| Error::Network(e.to_string()))?;
    let len = u16::from_be_bytes(len_buf) as usize;
    if !(1..=MAX_HELLO_BYTES).contains(&len) {
        return Err(Error::Protocol(format!("invalid USB hello size {len}")));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await.map_err(|e| Error::Network(e.to_string()))?;
    let hello: UsbHello =
        serde_json::from_slice(&payload).map_err(|e| Error::Protocol(format!("invalid USB hello: {e}")))?;
    let peer_id: DeviceId =
        hello.id.parse().map_err(|_| Error::Protocol("invalid DeviceId in USB hello".into()))?;
    let name: String = hello.name.trim().chars().take(64).collect();
    Ok((peer_id, name))
}

async fn exchange_hello(
    stream: &mut TcpStream,
    local_id: &DeviceId,
    local_name: &str,
) -> Result<(DeviceId, String)> {
    tokio::time::timeout(HELLO_TIMEOUT, async {
        write_hello(stream, local_id, local_name).await?;
        read_hello(stream).await
    })
    .await
    .map_err(|_| Error::Timeout)?
}

/// Starts the phone-side loopback TCP listener (`127.0.0.1:<preferred_port>`,
/// falling back to an ephemeral port if busy). Returns the bound TCP port.
pub(crate) fn start_phone_listener(shared: &Arc<Shared>, preferred_port: u16) -> Option<u16> {
    let std_listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, preferred_port))
        .or_else(|_| {
            if preferred_port != 0 {
                std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            } else {
                Err(std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "bind failed"))
            }
        })
        .ok()?;
    std_listener.set_nonblocking(true).ok()?;
    let port = std_listener.local_addr().ok()?.port();
    let listener = TcpListener::from_std(std_listener).ok()?;
    shared.usb_listener_port.store(port, Ordering::Relaxed);

    let weak = Arc::downgrade(shared);
    let cancel = shared.cancel.clone();
    tokio::spawn(async move {
        loop {
            let accepted = tokio::select! {
                _ = cancel.cancelled() => return,
                res = listener.accept() => res,
            };
            let (mut stream, peer_addr) = match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::debug!(error = %e, "USB listener accept failed");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };
            if !peer_addr.ip().to_canonical().is_loopback() {
                continue;
            }
            let _ = stream.set_nodelay(true);
            let Some(node) = weak.upgrade() else { return };
            tokio::spawn(async move {
                let local_name = node.local.read().unwrap_or_else(|e| e.into_inner()).device.name.clone();
                let Ok(Ok((pc_id, pc_name))) =
                    tokio::time::timeout(HELLO_TIMEOUT, read_hello(&mut stream)).await
                else {
                    return;
                };
                if pc_id == node.id {
                    return;
                }
                let Some(iroh_port) = node.bound_v4_port() else { return };
                let Ok(udp) = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await else { return };
                let Ok(bridge_addr) = udp.local_addr() else { return };
                let bridge_port = bridge_addr.port();

                let tunnel_cancel = node.cancel.child_token();
                node.register_usb_bridge_port(bridge_port);
                node.on_usb_tunnel_accepted(pc_id, pc_name, bridge_addr, tunnel_cancel.clone());

                if !matches!(
                    tokio::time::timeout(HELLO_TIMEOUT, write_hello(&mut stream, &node.id, &local_name))
                        .await,
                    Ok(Ok(()))
                ) {
                    node.unregister_usb_bridge_port(bridge_port);
                    node.on_usb_tunnel_disconnected(pc_id, bridge_addr).await;
                    return;
                }

                let default_target = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), iroh_port);
                relay_tcp_udp(stream, udp, default_target, tunnel_cancel).await;

                node.unregister_usb_bridge_port(bridge_port);
                node.on_usb_tunnel_disconnected(pc_id, bridge_addr).await;
            });
        }
    });
    Some(port)
}

/// Connects to a forwarded phone USB tunnel TCP address (`127.0.0.1:<port>`),
/// runs the `NLQU` handshake, spawns the UDP-over-TCP bridge, and triggers
/// session connection (if paired) or USB auto-pairing (if unpaired).
pub(crate) async fn connect_usb_tunnel(shared: &Arc<Shared>, forwarded_tcp: SocketAddr) -> Result<DeviceId> {
    let mut stream = tokio::time::timeout(HELLO_TIMEOUT, TcpStream::connect(forwarded_tcp))
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|e| Error::Network(e.to_string()))?;
    let _ = stream.set_nodelay(true);

    let local_name = shared.local.read().unwrap_or_else(|e| e.into_inner()).device.name.clone();
    let (phone_id, phone_name) = exchange_hello(&mut stream, &shared.id, &local_name).await?;
    if phone_id == shared.id {
        return Err(Error::Protocol("cannot connect USB tunnel to self".into()));
    }
    let iroh_port =
        shared.bound_v4_port().ok_or_else(|| Error::Internal("no local IPv4 QUIC port".into()))?;
    let udp = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.map_err(|e| Error::Network(e.to_string()))?;
    let bridge_addr = udp.local_addr().map_err(|e| Error::Network(e.to_string()))?;
    let bridge_port = bridge_addr.port();

    let tunnel_cancel = shared.cancel.child_token();
    shared.register_usb_bridge_port(bridge_port);

    let node = shared.clone();
    let relay_cancel = tunnel_cancel.clone();
    let default_target = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), iroh_port);
    tokio::spawn(async move {
        relay_tcp_udp(stream, udp, default_target, relay_cancel).await;
        node.unregister_usb_bridge_port(bridge_port);
        node.on_usb_tunnel_disconnected(phone_id, bridge_addr).await;
    });

    shared.on_usb_tunnel_connected(phone_id, phone_name, bridge_addr, tunnel_cancel, true);
    Ok(phone_id)
}

/// Starts the PC-side ADB watcher that tracks USB-connected Android devices,
/// sets up `adb forward tcp:0 tcp:53319`, and maintains the USB tunnel.
pub(crate) fn start_adb_watcher(shared: &Arc<Shared>, adb_addr: SocketAddr) {
    let weak = Arc::downgrade(shared);
    let cancel = shared.cancel.clone();
    tokio::spawn(async move {
        let mut active: HashMap<String, CancellationToken> = HashMap::new();
        loop {
            if cancel.is_cancelled() {
                break;
            }
            let Some(node) = weak.upgrade() else { break };
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let track_cancel = cancel.child_token();
            let track_cancel_task = track_cancel.clone();
            tokio::spawn(async move {
                let _ = nectarlink_adb::track_devices(adb_addr, track_cancel_task, move |devices| {
                    let _ = tx.send(devices);
                })
                .await;
            });
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = node.usb_poke.notified() => {
                        if let Ok(devices) = nectarlink_adb::list_devices(adb_addr).await {
                            reconcile_adb_devices(&node, adb_addr, &devices, &mut active);
                        }
                    }
                    maybe_devices = rx.recv() => {
                        match maybe_devices {
                            Some(devices) => reconcile_adb_devices(&node, adb_addr, &devices, &mut active),
                            None => break,
                        }
                    }
                }
            }
            track_cancel.cancel();
            drop(node);
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(ADB_RETRY_DELAY) => {}
            }
        }
        for (_, token) in active.drain() {
            token.cancel();
        }
    });
}

fn reconcile_adb_devices(
    shared: &Arc<Shared>,
    adb_addr: SocketAddr,
    devices: &[nectarlink_adb::AdbDevice],
    active: &mut HashMap<String, CancellationToken>,
) {
    active.retain(|serial, token| {
        let still_online = devices
            .iter()
            .any(|d| d.serial == *serial && d.is_online() && nectarlink_adb::is_serial_allowed(&d.serial));
        if !still_online {
            token.cancel();
        }
        still_online
    });

    for dev in devices {
        if !dev.is_online() || !nectarlink_adb::is_serial_allowed(&dev.serial) {
            continue;
        }
        if active.contains_key(&dev.serial) {
            continue;
        }
        let token = shared.cancel.child_token();
        active.insert(dev.serial.clone(), token.clone());
        let node = shared.clone();
        let serial = dev.serial.clone();
        tokio::spawn(async move {
            run_serial_worker(node, adb_addr, serial, token).await;
        });
    }
}

async fn run_serial_worker(
    shared: Arc<Shared>,
    adb_addr: SocketAddr,
    serial: String,
    cancel: CancellationToken,
) {
    while !cancel.is_cancelled() {
        if !nectarlink_adb::is_serial_allowed(&serial) {
            return;
        }
        if let Ok(host_port) = nectarlink_adb::forward_tcp(adb_addr, &serial, 0, DEFAULT_USB_PORT).await {
            let forwarded = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), host_port);
            run_forwarded_session(&shared, forwarded, &cancel, auto_pairs(&serial)).await;
            let _ = nectarlink_adb::kill_forward_tcp(adb_addr, &serial, host_port).await;
        }
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = shared.usb_poke.notified() => {}
            _ = tokio::time::sleep(ADB_RETRY_DELAY) => {}
        }
    }
}

/// Whether plugging in this device should start pairing on its own. Android
/// emulators show up in adb like phones, but a developer running one almost
/// never means to pair it, and the prompt would come back on every restart.
/// They can still be paired by hand.
fn auto_pairs(serial: &str) -> bool {
    !serial.trim().starts_with("emulator-")
}

async fn run_forwarded_session(
    shared: &Arc<Shared>,
    forwarded_tcp: SocketAddr,
    cancel: &CancellationToken,
    auto_pair: bool,
) {
    let Ok(Ok(mut stream)) = tokio::time::timeout(HELLO_TIMEOUT, TcpStream::connect(forwarded_tcp)).await
    else {
        return;
    };
    let _ = stream.set_nodelay(true);
    let local_name = shared.local.read().unwrap_or_else(|e| e.into_inner()).device.name.clone();
    let Ok((phone_id, phone_name)) = exchange_hello(&mut stream, &shared.id, &local_name).await else {
        return;
    };
    if phone_id == shared.id {
        return;
    }
    let Some(iroh_port) = shared.bound_v4_port() else { return };
    let Ok(udp) = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await else { return };
    let Ok(bridge_addr) = udp.local_addr() else { return };
    let bridge_port = bridge_addr.port();

    let tunnel_cancel = cancel.child_token();
    shared.register_usb_bridge_port(bridge_port);
    shared.on_usb_tunnel_connected(phone_id, phone_name, bridge_addr, tunnel_cancel.clone(), auto_pair);

    let default_target = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), iroh_port);
    relay_tcp_udp(stream, udp, default_target, tunnel_cancel).await;

    shared.unregister_usb_bridge_port(bridge_port);
    shared.on_usb_tunnel_disconnected(phone_id, bridge_addr).await;
}

/// Bridges UDP datagrams on `udp` to length-prefixed frames (`u16` big-endian)
/// on `stream`. Pre-initializes the local UDP peer target to `default_target`
/// (`127.0.0.1:<iroh_port>`) and updates it whenever a local UDP datagram
/// arrives.
async fn relay_tcp_udp(
    stream: TcpStream,
    udp: UdpSocket,
    default_target: SocketAddr,
    cancel: CancellationToken,
) {
    let (mut tcp_read, mut tcp_write) = stream.into_split();
    let udp_recv = Arc::new(udp);
    let udp_send = udp_recv.clone();
    let target = Arc::new(Mutex::new(default_target));
    let target_for_recv = target.clone();

    let udp_to_tcp = async move {
        let mut buf = vec![0u8; 65535];
        loop {
            let (n, from) = udp_recv.recv_from(&mut buf).await?;
            if n == 0 || n > u16::MAX as usize {
                continue;
            }
            *target_for_recv.lock().unwrap_or_else(|e| e.into_inner()) = from;
            tcp_write.write_all(&(n as u16).to_be_bytes()).await?;
            tcp_write.write_all(&buf[..n]).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), std::io::Error>(())
    };

    let tcp_to_udp = async move {
        let mut buf = vec![0u8; 65535];
        loop {
            let mut len_buf = [0u8; 2];
            tcp_read.read_exact(&mut len_buf).await?;
            let len = u16::from_be_bytes(len_buf) as usize;
            if len == 0 {
                continue;
            }
            tcp_read.read_exact(&mut buf[..len]).await?;
            let dest = *target.lock().unwrap_or_else(|e| e.into_inner());
            let _ = udp_send.send_to(&buf[..len], dest).await;
        }
        #[allow(unreachable_code)]
        Ok::<(), std::io::Error>(())
    };

    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = udp_to_tcp => {}
        _ = tcp_to_udp => {}
    }
}

#[cfg(test)]
mod tests {
    use super::auto_pairs;

    #[test]
    fn emulators_are_not_auto_paired() {
        assert!(!auto_pairs("emulator-5554"));
        assert!(!auto_pairs(" emulator-5556 "));
        assert!(auto_pairs("R58M123ABC"));
        assert!(auto_pairs("192.168.1.20:37099"));
        assert!(auto_pairs("adb-R58M123ABC-xyz._adb-tls-connect._tcp"));
    }
}
