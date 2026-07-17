use anyhow::Result;
use dhcproto::{Decodable, Encodable, v4};
use packet_crafter::{Packet, headers::Header};
use std::net::Ipv4Addr;

const ROUTER_ADDR: Ipv4Addr = Ipv4Addr::new(10, 1, 10, 1);
const CLIENT_ADDR: Ipv4Addr = Ipv4Addr::new(10, 1, 10, 2);
const DNS_SERVERS: [Ipv4Addr; 2] = [Ipv4Addr::new(223, 5, 5, 5), Ipv4Addr::new(119, 29, 29, 29)];

pub fn maybe_build_reply(packet: &[u8]) -> Result<Option<Vec<u8>>> {
    let parsed = match Packet::parse(packet) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let Some(ip) = parsed.get_ip_header() else {
        return Ok(None);
    };
    let Some(udp) = parsed.get_udp_header() else {
        return Ok(None);
    };
    if *udp.get_src_port() != 68 || *udp.get_dst_port() != 67 {
        return Ok(None);
    }
    let offset = (ip.get_length() + 8) as usize;
    if packet.len() <= offset {
        return Ok(None);
    }
    let payload = packet[offset..].to_vec();
    let mut request = v4::Message::decode(&mut v4::Decoder::new(&payload))?;
    if request.opcode() != v4::Opcode::BootRequest {
        return Ok(None);
    }
    request.set_opcode(v4::Opcode::BootReply);
    request.set_secs(0);
    request.set_flags(0.into());
    request.set_ciaddr(Ipv4Addr::UNSPECIFIED);
    request.set_yiaddr(CLIENT_ADDR);
    request.set_siaddr(ROUTER_ADDR);
    request.set_giaddr(Ipv4Addr::UNSPECIFIED);

    let mut options = v4::DhcpOptions::new();
    match request.opts().get(v4::OptionCode::MessageType) {
        Some(v4::DhcpOption::MessageType(v4::MessageType::Discover)) => {
            options.insert(v4::DhcpOption::MessageType(v4::MessageType::Offer));
        }
        Some(v4::DhcpOption::MessageType(v4::MessageType::Request)) => {
            options.insert(v4::DhcpOption::MessageType(v4::MessageType::Ack));
        }
        _ => {}
    }
    options.insert(v4::DhcpOption::SubnetMask(Ipv4Addr::new(255, 255, 255, 0)));
    options.insert(v4::DhcpOption::Router(vec![ROUTER_ADDR]));
    options.insert(v4::DhcpOption::DomainNameServer(DNS_SERVERS.to_vec()));
    options.insert(v4::DhcpOption::AddressLeaseTime(269_352_960));
    options.insert(v4::DhcpOption::ServerIdentifier(ROUTER_ADDR));
    request.set_opts(options);

    let mut dhcp = Vec::new();
    request.encode(&mut v4::Encoder::new(&mut dhcp))?;
    Ok(Some(build_udp_ipv4(
        Ipv4Addr::BROADCAST,
        ROUTER_ADDR,
        67,
        68,
        &dhcp,
    )))
}

fn build_udp_ipv4(
    source: Ipv4Addr,
    destination: Ipv4Addr,
    source_port: u16,
    destination_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    let udp_len = (8 + payload.len()) as u16;
    let mut udp = Vec::with_capacity(udp_len as usize);
    push_u16(&mut udp, source_port);
    push_u16(&mut udp, destination_port);
    push_u16(&mut udp, udp_len);
    push_u16(&mut udp, 0);
    udp.extend_from_slice(payload);

    let mut pseudo = Vec::with_capacity(12 + udp.len());
    pseudo.extend_from_slice(&source.octets());
    pseudo.extend_from_slice(&destination.octets());
    pseudo.push(0);
    pseudo.push(17);
    push_u16(&mut pseudo, udp_len);
    pseudo.extend_from_slice(&udp);
    udp[6..8].copy_from_slice(&checksum(&pseudo).to_be_bytes());

    let mut ip = Vec::with_capacity(20 + udp.len());
    ip.extend_from_slice(&[0x45, 0]);
    push_u16(&mut ip, (20 + udp.len()) as u16);
    ip.extend_from_slice(&[0, 0, 0, 0, 64, 17, 0, 0]);
    ip.extend_from_slice(&source.octets());
    ip.extend_from_slice(&destination.octets());
    let ip_checksum = checksum(&ip);
    ip[10..12].copy_from_slice(&ip_checksum.to_be_bytes());
    ip.extend_from_slice(&udp);
    ip
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn checksum(bytes: &[u8]) -> u16 {
    let mut sum = 0u32;
    for chunk in bytes.chunks(2) {
        sum += if chunk.len() == 2 {
            u16::from_be_bytes([chunk[0], chunk[1]]) as u32
        } else {
            (chunk[0] as u32) << 8
        };
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_a_discover_into_an_offer() {
        let mut discover = v4::Message::default();
        discover
            .opts_mut()
            .insert(v4::DhcpOption::MessageType(v4::MessageType::Discover));
        let mut payload = Vec::new();
        discover
            .encode(&mut v4::Encoder::new(&mut payload))
            .expect("encode DHCP discover");
        let request = build_udp_ipv4(Ipv4Addr::UNSPECIFIED, Ipv4Addr::BROADCAST, 68, 67, &payload);

        let response = maybe_build_reply(&request)
            .expect("parse DHCP discover")
            .expect("build DHCP offer");
        let offer =
            v4::Message::decode(&mut v4::Decoder::new(&response[28..])).expect("decode DHCP offer");

        assert_eq!(offer.opcode(), v4::Opcode::BootReply);
        assert_eq!(offer.yiaddr(), CLIENT_ADDR);
        assert!(matches!(
            offer.opts().get(v4::OptionCode::MessageType),
            Some(v4::DhcpOption::MessageType(v4::MessageType::Offer))
        ));
        assert!(matches!(
            offer.opts().get(v4::OptionCode::Router),
            Some(v4::DhcpOption::Router(routers)) if routers == &vec![ROUTER_ADDR]
        ));
    }
}
