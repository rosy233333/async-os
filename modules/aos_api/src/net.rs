use async_io::PollState as AxPollState;
use async_net::{TcpSocket, UdpSocket};
use axerrno::AxResult;
use core::net::{IpAddr, SocketAddr};

/// A handle to a TCP socket.
pub struct AxTcpSocketHandle(TcpSocket);

/// A handle to a UDP socket.
pub struct AxUdpSocketHandle(UdpSocket);

////////////////////////////////////////////////////////////////////////////////
// TCP socket
////////////////////////////////////////////////////////////////////////////////

pub fn ax_tcp_socket() -> AxTcpSocketHandle {
    AxTcpSocketHandle(TcpSocket::new())
}

pub fn ax_tcp_socket_addr(socket: &AxTcpSocketHandle) -> AxResult<SocketAddr> {
    socket.0.local_addr()
}

pub fn ax_tcp_peer_addr(socket: &AxTcpSocketHandle) -> AxResult<SocketAddr> {
    socket.0.peer_addr()
}

pub fn ax_tcp_set_nonblocking(socket: &AxTcpSocketHandle, nonblocking: bool) -> AxResult {
    socket.0.set_nonblocking(nonblocking);
    Ok(())
}

pub async fn ax_tcp_connect(socket: &AxTcpSocketHandle, addr: SocketAddr) -> AxResult {
    socket.0.connect(addr).await
}

pub async fn ax_tcp_bind(socket: &AxTcpSocketHandle, addr: SocketAddr) -> AxResult {
    socket.0.bind(addr).await
}

pub async fn ax_tcp_listen(socket: &AxTcpSocketHandle, _backlog: usize) -> AxResult {
    socket.0.listen().await
}

pub async fn ax_tcp_accept(
    socket: &AxTcpSocketHandle,
) -> AxResult<(AxTcpSocketHandle, SocketAddr)> {
    let new_sock = socket.0.accept().await?;
    let addr = new_sock.peer_addr()?;
    Ok((AxTcpSocketHandle(new_sock), addr))
}

pub async fn ax_tcp_send(socket: &AxTcpSocketHandle, buf: &[u8]) -> AxResult<usize> {
    socket.0.send(buf).await
}

pub async fn ax_tcp_recv(socket: &AxTcpSocketHandle, buf: &mut [u8]) -> AxResult<usize> {
    socket.0.recv(buf).await
}

pub async fn ax_tcp_poll(socket: &AxTcpSocketHandle) -> AxResult<AxPollState> {
    socket.0.poll().await
}

pub async fn ax_tcp_shutdown(socket: &AxTcpSocketHandle) -> AxResult {
    socket.0.shutdown().await
}

////////////////////////////////////////////////////////////////////////////////
// UDP socket
////////////////////////////////////////////////////////////////////////////////

pub async fn ax_udp_socket() -> AxUdpSocketHandle {
    AxUdpSocketHandle(UdpSocket::new().await)
}

pub fn ax_udp_socket_addr(socket: &AxUdpSocketHandle) -> AxResult<SocketAddr> {
    socket.0.local_addr()
}

pub fn ax_udp_peer_addr(socket: &AxUdpSocketHandle) -> AxResult<SocketAddr> {
    socket.0.peer_addr()
}

pub fn ax_udp_set_nonblocking(socket: &AxUdpSocketHandle, nonblocking: bool) -> AxResult {
    socket.0.set_nonblocking(nonblocking);
    Ok(())
}

pub async fn ax_udp_bind(socket: &AxUdpSocketHandle, addr: SocketAddr) -> AxResult {
    socket.0.bind(addr).await
}

pub async fn ax_udp_recv_from(
    socket: &AxUdpSocketHandle,
    buf: &mut [u8],
) -> AxResult<(usize, SocketAddr)> {
    socket.0.recv_from(buf).await
}

pub async fn ax_udp_peek_from(
    socket: &AxUdpSocketHandle,
    buf: &mut [u8],
) -> AxResult<(usize, SocketAddr)> {
    socket.0.peek_from(buf).await
}

pub async fn ax_udp_send_to(
    socket: &AxUdpSocketHandle,
    buf: &[u8],
    addr: SocketAddr,
) -> AxResult<usize> {
    socket.0.send_to(buf, addr).await
}

pub async fn ax_udp_connect(socket: &AxUdpSocketHandle, addr: SocketAddr) -> AxResult {
    socket.0.connect(addr).await
}

pub async fn ax_udp_send(socket: &AxUdpSocketHandle, buf: &[u8]) -> AxResult<usize> {
    socket.0.send(buf).await
}

pub async fn ax_udp_recv(socket: &AxUdpSocketHandle, buf: &mut [u8]) -> AxResult<usize> {
    socket.0.recv(buf).await
}

pub async fn ax_udp_poll(socket: &AxUdpSocketHandle) -> AxResult<AxPollState> {
    socket.0.poll().await
}

////////////////////////////////////////////////////////////////////////////////
// Miscellaneous
////////////////////////////////////////////////////////////////////////////////

pub async fn ax_dns_query(domain_name: &str) -> AxResult<alloc::vec::Vec<IpAddr>> {
    async_net::dns_query(domain_name).await
}

pub async fn ax_poll_interfaces() -> AxResult {
    async_net::poll_interfaces().await;
    Ok(())
}
