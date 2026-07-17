use crate::{dhcp::maybe_build_reply, meter::BandwidthMeter, tun::MiWearTunDevice};
use anyhow::{Context, Result};
use etherparse::{Icmpv4Header, Icmpv4Type};
use ipstack::{IpNumber, IpStack, IpStackConfig, IpStackStream};
use parking_lot::Mutex;
use pcap_file::pcap::PcapWriter;
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::File,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{self, AsyncWriteExt},
    net::TcpStream,
    runtime::Handle,
    sync::mpsc,
    task::JoinHandle,
};
use tokio_util::{sync::CancellationToken, sync::PollSender};
use udp_stream::UdpStream;

pub type WakeCallback = unsafe extern "C" fn(u64);

#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub mtu: u16,
    pub ingress_capacity: usize,
    pub stack_capacity: usize,
    pub outbound_capacity: usize,
    pub meter_window: Duration,
    pub stats_interval: Duration,
    pub capture_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[repr(u32)]
pub enum EventKind {
    Packet = 1,
    State = 2,
    Statistics = 3,
    Warning = 4,
}

pub struct NetworkEvent {
    pub kind: EventKind,
    pub payload: Vec<u8>,
}

#[derive(Default)]
pub struct NetworkCounters {
    pub bytes_from_device: AtomicU64,
    pub bytes_to_device: AtomicU64,
    pub active_sessions: AtomicUsize,
    pub dropped_packets: AtomicU64,
}

#[derive(Serialize)]
struct StatisticsEvent {
    bytes_from_device: u64,
    bytes_to_device: u64,
    read_bytes_per_second: f64,
    write_bytes_per_second: f64,
    active_sessions: usize,
    dropped_packets: u64,
}

pub struct NetworkSession {
    handle: AtomicU64,
    ingress: mpsc::Sender<Vec<u8>>,
    events: Mutex<VecDeque<NetworkEvent>>,
    callback: WakeCallback,
    callback_lock: Mutex<()>,
    cancellation: CancellationToken,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    pub counters: Arc<NetworkCounters>,
    max_events: usize,
    alive: AtomicBool,
}

impl NetworkSession {
    pub fn start(
        config: NetworkConfig,
        callback: WakeCallback,
        runtime: &Handle,
    ) -> Result<Arc<Self>> {
        let (ingress_tx, mut ingress_rx) = mpsc::channel(config.ingress_capacity.max(1));
        let (stack_tx, stack_rx) = mpsc::channel(config.stack_capacity.max(1));
        let (outbound_tx, mut outbound_rx) = mpsc::channel(config.outbound_capacity.max(1));
        let cancellation = CancellationToken::new();
        let counters = Arc::new(NetworkCounters::default());
        let meter = BandwidthMeter::new(config.meter_window);
        let session = Arc::new(Self {
            handle: AtomicU64::new(0),
            ingress: ingress_tx,
            events: Default::default(),
            callback,
            callback_lock: Default::default(),
            cancellation: cancellation.clone(),
            tasks: Default::default(),
            counters: counters.clone(),
            max_events: config.outbound_capacity.max(1),
            alive: AtomicBool::new(true),
        });

        let inbound_session = session.clone();
        let inbound_cancel = cancellation.clone();
        let inbound_task = runtime.spawn(async move {
            loop {
                tokio::select! {
                    _ = inbound_cancel.cancelled() => break,
                    packet = ingress_rx.recv() => match packet {
                        Some(packet) => match maybe_build_reply(&packet) {
                            Ok(Some(reply)) => inbound_session.emit_packet(reply),
                            Ok(None) => {
                                if stack_tx.send(packet).await.is_err() { break; }
                            }
                            Err(error) => inbound_session.emit_text(EventKind::Warning, format!("DHCP parse failed: {error:#}")),
                        },
                        None => break,
                    }
                }
            }
        });

        let outbound_session = session.clone();
        let outbound_cancel = cancellation.clone();
        let outbound_task = runtime.spawn(async move {
            loop {
                tokio::select! {
                    _ = outbound_cancel.cancelled() => break,
                    packet = outbound_rx.recv() => match packet {
                        Some(packet) => outbound_session.emit_packet(packet),
                        None => break,
                    }
                }
            }
        });

        let stack_session = session.clone();
        let stack_cancel = cancellation.clone();
        let stack_meter = meter.clone();
        let stack_config = config.clone();
        let stack_task = runtime.spawn(async move {
            let capture = match open_capture(stack_config.capture_path.as_ref()) {
                Ok(capture) => capture,
                Err(error) => {
                    stack_session.emit_text(
                        EventKind::Warning,
                        format!("PCAP capture is unavailable: {error:#}"),
                    );
                    None
                }
            };
            let tun = MiWearTunDevice {
                inbound: stack_rx,
                outbound: PollSender::new(outbound_tx),
                capture,
                meter: stack_meter,
            };
            let mut ip_config = IpStackConfig::default();
            ip_config.mtu(stack_config.mtu);
            let mut stack = IpStack::new(ip_config, tun);
            stack_session.emit_text(EventKind::State, "network stack started");
            loop {
                tokio::select! {
                    _ = stack_cancel.cancelled() => break,
                    accepted = stack.accept() => match accepted {
                        Ok(stream) => spawn_stream(stream, stack_session.clone(), stack_cancel.clone()),
                        Err(error) => {
                            stack_session.emit_text(EventKind::Warning, format!("IP stack stopped: {error}"));
                            break;
                        }
                    }
                }
            }
        });

        let stats_session = session.clone();
        let stats_cancel = cancellation;
        let stats_task = runtime.spawn(async move {
            let mut interval =
                tokio::time::interval(config.stats_interval.max(Duration::from_millis(100)));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = stats_cancel.cancelled() => break,
                    _ = interval.tick() => {
                        let payload = StatisticsEvent {
                            bytes_from_device: counters.bytes_from_device.load(Ordering::Relaxed),
                            bytes_to_device: counters.bytes_to_device.load(Ordering::Relaxed),
                            read_bytes_per_second: meter.read_speed(),
                            write_bytes_per_second: meter.write_speed(),
                            active_sessions: counters.active_sessions.load(Ordering::Relaxed),
                            dropped_packets: counters.dropped_packets.load(Ordering::Relaxed),
                        };
                        if let Ok(payload) = serde_json::to_vec(&payload) {
                            stats_session.emit(EventKind::Statistics, payload);
                        }
                    }
                }
            }
        });

        session
            .tasks
            .lock()
            .extend([inbound_task, outbound_task, stack_task, stats_task]);
        Ok(session)
    }

    pub fn set_handle(&self, handle: u64) {
        self.handle.store(handle, Ordering::Release);
    }

    pub fn push_inbound(&self, packet: &[u8]) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            anyhow::bail!("network session is closed");
        }
        self.counters
            .bytes_from_device
            .fetch_add(packet.len() as u64, Ordering::Relaxed);
        self.ingress.try_send(packet.to_vec()).map_err(|error| {
            self.counters
                .dropped_packets
                .fetch_add(1, Ordering::Relaxed);
            anyhow::anyhow!("network ingress rejected: {error}")
        })
    }

    pub fn peek_event(&self) -> Option<(EventKind, usize)> {
        self.events
            .lock()
            .front()
            .map(|event| (event.kind, event.payload.len()))
    }

    pub fn pop_event(&self) -> Option<NetworkEvent> {
        self.events.lock().pop_front()
    }

    pub fn close(&self) {
        if !self.alive.swap(false, Ordering::AcqRel) {
            return;
        }
        let _callback_guard = self.callback_lock.lock();
        self.cancellation.cancel();
        for task in self.tasks.lock().drain(..) {
            task.abort();
        }
        self.events.lock().clear();
    }

    fn emit_packet(&self, packet: Vec<u8>) {
        let length = packet.len() as u64;
        if self.emit(EventKind::Packet, packet) {
            self.counters
                .bytes_to_device
                .fetch_add(length, Ordering::Relaxed);
        }
    }

    fn emit_text(&self, kind: EventKind, message: impl Into<String>) {
        self.emit(kind, message.into().into_bytes());
    }

    fn emit(&self, kind: EventKind, payload: Vec<u8>) -> bool {
        let _callback_guard = self.callback_lock.lock();
        if !self.alive.load(Ordering::Acquire) {
            return false;
        }
        let mut events = self.events.lock();
        if events.len() >= self.max_events {
            self.counters
                .dropped_packets
                .fetch_add(1, Ordering::Relaxed);
            return false;
        }
        events.push_back(NetworkEvent { kind, payload });
        drop(events);
        let handle = self.handle.load(Ordering::Acquire);
        if handle != 0 {
            unsafe { (self.callback)(handle) };
        }
        true
    }
}

impl Drop for NetworkSession {
    fn drop(&mut self) {
        self.close();
    }
}

fn spawn_stream(
    stream: IpStackStream,
    session: Arc<NetworkSession>,
    cancellation: CancellationToken,
) {
    match stream {
        IpStackStream::Tcp(mut device) => {
            tokio::spawn(async move {
                let peer_address = device.peer_addr();
                let mut peer = match TcpStream::connect(peer_address).await {
                    Ok(peer) => peer,
                    Err(error) => {
                        session.emit_text(
                            EventKind::Warning,
                            format!("TCP connect {peer_address} failed: {error}"),
                        );
                        return;
                    }
                };
                let _guard = SessionGuard::new(session.counters.clone());
                tokio::select! {
                    _ = cancellation.cancelled() => {},
                    result = io::copy_bidirectional(&mut device, &mut peer) => {
                        if let Err(error) = result {
                            session.emit_text(EventKind::Warning, format!("TCP {peer_address} closed: {error}"));
                        }
                    }
                }
                let _ = peer.shutdown().await;
                let _ = device.shutdown().await;
            });
        }
        IpStackStream::Udp(mut device) => {
            tokio::spawn(async move {
                let local = device.local_addr();
                let remote = device.peer_addr();
                let mut peer = match UdpStream::connect(remote).await {
                    Ok(peer) => peer,
                    Err(error) => {
                        session.emit_text(
                            EventKind::Warning,
                            format!("UDP {local} -> {remote} failed: {error}"),
                        );
                        return;
                    }
                };
                let _guard = SessionGuard::new(session.counters.clone());
                tokio::select! {
                    _ = cancellation.cancelled() => {},
                    result = io::copy_bidirectional(&mut device, &mut peer) => {
                        if let Err(error) = result {
                            session.emit_text(EventKind::Warning, format!("UDP {local} -> {remote} closed: {error}"));
                        }
                    }
                }
                peer.shutdown();
                let _ = device.shutdown().await;
            });
        }
        IpStackStream::UnknownTransport(packet) => {
            if packet.src_addr().is_ipv4()
                && packet.ip_protocol() == IpNumber::ICMP
                && let Ok((header, payload)) = Icmpv4Header::from_slice(packet.payload())
                && let Icmpv4Type::EchoRequest(echo) = header.icmp_type
            {
                let mut response = Icmpv4Header::new(Icmpv4Type::EchoReply(echo));
                response.update_checksum(payload);
                let mut bytes = response.to_bytes().to_vec();
                bytes.extend_from_slice(payload);
                if let Err(error) = packet.send(bytes) {
                    session.emit_text(EventKind::Warning, format!("ICMP reply failed: {error}"));
                }
            }
        }
        IpStackStream::UnknownNetwork(packet) => {
            session.emit_text(
                EventKind::Warning,
                format!("unsupported network packet ({} bytes)", packet.len()),
            );
        }
    }
}

struct SessionGuard(Arc<NetworkCounters>);

impl SessionGuard {
    fn new(counters: Arc<NetworkCounters>) -> Self {
        counters.active_sessions.fetch_add(1, Ordering::Relaxed);
        Self(counters)
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.active_sessions.fetch_sub(1, Ordering::Relaxed);
    }
}

fn open_capture(path: Option<&PathBuf>) -> Result<Option<PcapWriter<File>>> {
    let Some(path) = path else { return Ok(None) };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create capture directory {}", parent.display()))?;
    }
    let file =
        File::create(path).with_context(|| format!("create capture file {}", path.display()))?;
    Ok(Some(PcapWriter::new(file).context("write PCAP header")?))
}
