use std::{net::{Ipv4Addr, Ipv6Addr, SocketAddr}};

use tokio::{net::UdpSocket};
use simple_dns::{PacketBuf, PacketHeader, SimpleDnsError};

mod oneshot_resolver;
mod simple_responder;

pub use oneshot_resolver::OneShotMdnsResolver;
pub use simple_responder::SimpleMdnsResponder;

const ENABLE_LOOPBACK: bool = cfg!(test);
const UNICAST_RESPONSE: bool = cfg!(not(test));


const MULTICAST_ADDR_IPV4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
// const MULTICAST_ADDR_IPV6: Ipv6Addr = Ipv6Addr::new(0xFF02, 0, 0, 0, 0, 0, 0, 0xFB);
const MULTICAST_PORT: u16 = 5353;

#[derive(Debug)]
pub enum SimpleMdnsError {
    ErrorCreatingUDPSocket,
    ErrorSendingDNSPacket,
    ErrorReadingFromUDPSocket,
    DnsParsing(SimpleDnsError)
}

impl From<SimpleDnsError> for SimpleMdnsError {
    fn from(inner: SimpleDnsError) -> Self {
        SimpleMdnsError::DnsParsing(inner)
    }
}


async fn send_packet_to_multicast_socket(socket: &UdpSocket, packet: &PacketBuf) -> Result<(), SimpleMdnsError>{

    // TODO: also send to ipv6
    let target_addr = std::net::SocketAddr::new(MULTICAST_ADDR_IPV4.into(), MULTICAST_PORT);
    socket.send_to(&packet, target_addr)
        .await
        .map_err(|_| SimpleMdnsError::ErrorSendingDNSPacket)?;

    Ok(())
}


async fn get_first_response(socket: &tokio::net::UdpSocket, packet_id: u16) -> Result<PacketBuf, SimpleMdnsError> {
    let mut buf = [0u8; 9000];
    
    loop {
        let (count, _) = socket.recv_from(&mut buf[..])
            .await
            .map_err(|_| SimpleMdnsError::ErrorReadingFromUDPSocket)?;

        if PacketHeader::id(&buf) == packet_id && PacketHeader::read_answers(&buf) > 0 {
            return Ok(buf[..count].into())
        }
    }
}

fn create_udp_socket(multicast_loop: bool) -> Result<tokio::net::UdpSocket, Box<dyn std::error::Error>> {
    // let addrs = [
    //     SocketAddr::from(([0, 0, 0, 0], MULTICAST_PORT)),
    //     // SocketAddr::from(([0, 0, 0, 0], 0)),
    // ];

    let socket = socket2::Socket::new(socket2::Domain::ipv4(), socket2::Type::dgram(), None).unwrap();
    socket.set_multicast_loop_v4(multicast_loop)?;
    socket.join_multicast_v4(&MULTICAST_ADDR_IPV4, &Ipv4Addr::new(0, 0, 0, 0))?;
    socket.set_reuse_address(true)?;
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    
    socket.bind(&SocketAddr::from(([0, 0, 0, 0], MULTICAST_PORT)).into())?;
    
    let socket = tokio::net::UdpSocket::from_std(socket.into_udp_socket())?;
    Ok(socket)
}
