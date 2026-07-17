use crate::meter::BandwidthMeter;
use pcap_file::pcap::{PcapPacket, PcapWriter};
use std::{
    fs::File,
    io::{Error, ErrorKind, Result},
    pin::Pin,
    task::{Context, Poll},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use tokio_util::sync::PollSender;

pub struct MiWearTunDevice {
    pub inbound: mpsc::Receiver<Vec<u8>>,
    pub outbound: PollSender<Vec<u8>>,
    pub capture: Option<PcapWriter<File>>,
    pub meter: BandwidthMeter,
}

impl AsyncRead for MiWearTunDevice {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        match self.inbound.poll_recv(cx) {
            Poll::Ready(Some(packet)) => {
                let length = packet.len().min(buf.remaining());
                buf.put_slice(&packet[..length]);
                self.meter.add_read(length);
                capture(&mut self.capture, false, &packet);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => Poll::Ready(Err(Error::new(
                ErrorKind::BrokenPipe,
                "network ingress closed",
            ))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for MiWearTunDevice {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        let packet = buf.to_vec();
        self.meter.add_write(packet.len());
        capture(&mut self.capture, true, &packet);
        match self.outbound.poll_reserve(cx) {
            Poll::Ready(Ok(())) => match self.outbound.send_item(packet) {
                Ok(()) => Poll::Ready(Ok(buf.len())),
                Err(_) => Poll::Ready(Err(Error::new(
                    ErrorKind::BrokenPipe,
                    "network egress closed",
                ))),
            },
            Poll::Ready(Err(_)) => Poll::Ready(Err(Error::new(
                ErrorKind::BrokenPipe,
                "network egress closed",
            ))),
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }
}

fn capture(writer: &mut Option<PcapWriter<File>>, outbound: bool, packet: &[u8]) {
    let Some(writer) = writer else { return };
    let mut ethernet = if outbound {
        hex("a5a5a5a5a5a50000000000000800")
    } else {
        hex("000000000000a5a5a5a5a5a50800")
    };
    ethernet.extend_from_slice(packet);
    let _ = writer.write_packet(&PcapPacket {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default(),
        orig_len: ethernet.len() as u32,
        data: ethernet.into(),
    });
}

fn hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).expect("hex"), 16).expect("hex"))
        .collect()
}
