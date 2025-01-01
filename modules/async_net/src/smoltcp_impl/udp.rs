use core::net::SocketAddr;
use core::ops::AsyncFnMut;
use core::sync::atomic::{AtomicBool, Ordering};

use alloc::boxed::Box;
use async_io::{AsyncRead, AsyncWrite, PollState};
use axerrno::{ax_err, ax_err_type, AxError, AxResult};
use axhal::time::current_ticks;
use spin::RwLock;
use sync::Mutex;

use smoltcp::iface::SocketHandle;
use smoltcp::socket::udp::{self, BindError, SendError};
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use super::addr::{from_core_sockaddr, into_core_sockaddr, is_unspecified, UNSPECIFIED_ENDPOINT};
use super::{SocketSetWrapper, SOCKET_SET};

/// A UDP socket that provides POSIX-like APIs.
pub struct UdpSocket {
    handle: SocketHandle,
    local_addr: RwLock<Option<IpEndpoint>>,
    peer_addr: RwLock<Option<IpEndpoint>>,
    nonblock: AtomicBool,
    reuse_addr: AtomicBool,
}

impl UdpSocket {
    /// Creates a new UDP socket.
    #[allow(clippy::new_without_default)]
    pub async fn new() -> Self {
        let socket = SocketSetWrapper::new_udp_socket();
        let handle = SOCKET_SET.add(socket).await;
        Self {
            handle,
            local_addr: RwLock::new(None),
            peer_addr: RwLock::new(None),
            nonblock: AtomicBool::new(false),
            reuse_addr: AtomicBool::new(false),
        }
    }

    /// Returns the local address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    pub fn local_addr(&self) -> AxResult<SocketAddr> {
        match self.local_addr.try_read() {
            Some(addr) => addr.map(into_core_sockaddr).ok_or(AxError::NotConnected),
            None => Err(AxError::NotConnected),
        }
    }

    /// Returns the remote address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    pub fn peer_addr(&self) -> AxResult<SocketAddr> {
        self.remote_endpoint().map(into_core_sockaddr)
    }

    /// Returns whether this socket is in nonblocking mode.
    #[inline]
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// Moves this UDP socket into or out of nonblocking mode.
    ///
    /// This will result in `recv`, `recv_from`, `send`, and `send_to`
    /// operations becoming nonblocking, i.e., immediately returning from their
    /// calls. If the IO operation is successful, `Ok` is returned and no
    /// further action is required. If the IO operation could not be completed
    /// and needs to be retried, an error with kind
    /// [`Err(WouldBlock)`](AxError::WouldBlock) is returned.
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    /// Set the TTL (time-to-live) option for this socket.
    ///
    /// The TTL is the number of hops that a packet is allowed to live.
    pub async fn set_socket_ttl(&self, ttl: u8) {
        SOCKET_SET
            .with_socket_mut::<udp::Socket, _, _>(self.handle, async |socket| {
                socket.set_hop_limit(Some(ttl))
            })
            .await;
    }

    /// Returns whether this socket is in reuse address mode.
    #[inline]
    pub fn is_reuse_addr(&self) -> bool {
        self.reuse_addr.load(Ordering::Acquire)
    }

    /// Moves this UDP socket into or out of reuse address mode.
    ///
    /// When a socket is bound, the `SO_REUSEADDR` option allows multiple sockets to be bound to the
    /// same address if they are bound to different local addresses. This option must be set before
    /// calling `bind`.
    #[inline]
    pub fn set_reuse_addr(&self, reuse_addr: bool) {
        self.reuse_addr.store(reuse_addr, Ordering::Release);
    }

    /// Binds an unbound socket to the given address and port.
    ///
    /// It's must be called before [`send_to`](Self::send_to) and
    /// [`recv_from`](Self::recv_from).
    pub async fn bind(&self, mut local_addr: SocketAddr) -> AxResult {
        let mut self_local_addr = self.local_addr.write();

        if local_addr.port() == 0 {
            local_addr.set_port(get_ephemeral_port().await?);
        }
        if self_local_addr.is_some() {
            return ax_err!(InvalidInput, "socket bind() failed: already bound");
        }

        let local_endpoint = from_core_sockaddr(local_addr);
        let endpoint = IpListenEndpoint {
            addr: (!is_unspecified(local_endpoint.addr)).then_some(local_endpoint.addr),
            port: local_endpoint.port,
        };

        if !self.is_reuse_addr() {
            // Check if the address is already in use
            SOCKET_SET
                .bind_check(local_endpoint.addr, local_endpoint.port)
                .await?;
        }

        SOCKET_SET
            .with_socket_mut::<udp::Socket, _, _>(self.handle, async |socket| {
                socket.bind(endpoint).or_else(|e| match e {
                    BindError::InvalidState => ax_err!(AlreadyExists, "socket bind() failed"),
                    BindError::Unaddressable => ax_err!(InvalidInput, "socket bind() failed"),
                })
            })
            .await?;

        *self_local_addr = Some(local_endpoint);
        debug!("UDP socket {}: bound on {}", self.handle, endpoint);
        Ok(())
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    pub async fn send_to(&self, buf: &[u8], remote_addr: SocketAddr) -> AxResult<usize> {
        if remote_addr.port() == 0 || remote_addr.ip().is_unspecified() {
            return ax_err!(InvalidInput, "socket send_to() failed: invalid address");
        }
        self.send_impl(buf, from_core_sockaddr(remote_addr)).await
    }

    /// Receives a single datagram message on the socket. On success, returns
    /// the number of bytes read and the origin.
    pub async fn recv_from(&self, buf: &mut [u8]) -> AxResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| match socket.recv_slice(buf) {
            Ok((len, meta)) => Ok((len, into_core_sockaddr(meta.endpoint))),
            Err(_) => ax_err!(BadState, "socket recv_from() failed"),
        })
        .await
    }

    /// Receives data from the socket, stores it in the given buffer.
    ///
    /// It will return [`Err(Timeout)`](AxError::Timeout) if expired.
    pub async fn recv_from_timeout(
        &self,
        buf: &mut [u8],
        ticks: u64,
    ) -> AxResult<(usize, SocketAddr)> {
        let expire_at = current_ticks() + ticks;
        self.recv_impl(|socket| match socket.recv_slice(buf) {
            Ok((len, meta)) => Ok((len, into_core_sockaddr(meta.endpoint))),
            Err(_) => {
                if current_ticks() > expire_at {
                    Err(AxError::Timeout)
                } else {
                    Err(AxError::WouldBlock)
                }
            }
        })
        .await
    }

    /// Receives a single datagram message on the socket, without removing it from
    /// the queue. On success, returns the number of bytes read and the origin.
    pub async fn peek_from(&self, buf: &mut [u8]) -> AxResult<(usize, SocketAddr)> {
        self.recv_impl(|socket| match socket.peek_slice(buf) {
            Ok((len, meta)) => Ok((len, into_core_sockaddr(meta.endpoint))),
            Err(_) => ax_err!(BadState, "socket recv_from() failed"),
        })
        .await
    }

    /// Connects this UDP socket to a remote address, allowing the `send` and
    /// `recv` to be used to send data and also applies filters to only receive
    /// data from the specified address.
    ///
    /// The local port will be generated automatically if the socket is not bound.
    /// It's must be called before [`send`](Self::send) and
    /// [`recv`](Self::recv).
    pub async fn connect(&self, addr: SocketAddr) -> AxResult {
        let mut self_peer_addr = self.peer_addr.write();

        if self.local_addr.read().is_none() {
            self.bind(into_core_sockaddr(UNSPECIFIED_ENDPOINT)).await?;
        }

        *self_peer_addr = Some(from_core_sockaddr(addr));
        debug!("UDP socket {}: connected to {}", self.handle, addr);
        Ok(())
    }

    /// Sends data on the socket to the remote address to which it is connected.
    pub async fn send(&self, buf: &[u8]) -> AxResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.send_impl(buf, remote_endpoint).await
    }

    /// Receives a single datagram message on the socket from the remote address
    /// to which it is connected. On success, returns the number of bytes read.
    pub async fn recv(&self, buf: &mut [u8]) -> AxResult<usize> {
        let remote_endpoint = self.remote_endpoint()?;
        self.recv_impl(|socket| {
            let (len, meta) = socket
                .recv_slice(buf)
                .map_err(|_| ax_err_type!(BadState, "socket recv() failed"))?;
            if !is_unspecified(remote_endpoint.addr) && remote_endpoint.addr != meta.endpoint.addr {
                return Err(AxError::WouldBlock);
            }
            if remote_endpoint.port != 0 && remote_endpoint.port != meta.endpoint.port {
                return Err(AxError::WouldBlock);
            }
            Ok(len)
        })
        .await
    }

    /// Close the socket.
    pub async fn shutdown(&self) -> AxResult {
        SOCKET_SET.poll_interfaces().await;
        SOCKET_SET
            .with_socket_mut::<udp::Socket, _, _>(self.handle, async |socket| {
                debug!("UDP socket {}: shutting down", self.handle);
                socket.close();
            })
            .await;
        Ok(())
    }

    /// Whether the socket is readable or writable.
    pub async fn poll(&self) -> AxResult<PollState> {
        if self.local_addr.read().is_none() {
            return Ok(PollState {
                readable: false,
                writable: false,
            });
        }
        SOCKET_SET
            .with_socket_mut::<udp::Socket, _, _>(self.handle, async |socket| {
                Ok(PollState {
                    readable: socket.can_recv(),
                    writable: socket.can_send(),
                })
            })
            .await
    }
}

/// Private methods
impl UdpSocket {
    fn remote_endpoint(&self) -> AxResult<IpEndpoint> {
        match self.peer_addr.try_read() {
            Some(addr) => addr.ok_or(AxError::NotConnected),
            None => Err(AxError::NotConnected),
        }
    }

    async fn send_impl(&self, buf: &[u8], remote_endpoint: IpEndpoint) -> AxResult<usize> {
        if self.local_addr.read().is_none() {
            return ax_err!(NotConnected, "socket send() failed");
        }
        // info!("send to addr: {:?}", remote_endpoint);
        self.block_on(async || {
            SOCKET_SET
                .with_socket_mut::<udp::Socket, _, _>(self.handle, async |socket| {
                    if !socket.is_open() {
                        // not connected
                        ax_err!(NotConnected, "socket send() failed")
                    } else if socket.can_send() {
                        socket
                            .send_slice(buf, remote_endpoint)
                            .map_err(|e| match e {
                                SendError::BufferFull => AxError::WouldBlock,
                                SendError::Unaddressable => {
                                    ax_err_type!(ConnectionRefused, "socket send() failed")
                                }
                            })?;
                        Ok(buf.len())
                    } else {
                        // tx buffer is full
                        Err(AxError::WouldBlock)
                    }
                })
                .await
        })
        .await
    }

    async fn recv_impl<F, T>(&self, mut op: F) -> AxResult<T>
    where
        F: FnMut(&mut udp::Socket) -> AxResult<T>,
    {
        if self.local_addr.read().is_none() {
            return ax_err!(NotConnected, "socket send() failed");
        }
        self.block_on(async || {
            SOCKET_SET
                .with_socket_mut::<udp::Socket, _, _>(self.handle, async |socket| {
                    if !socket.is_open() {
                        // not bound
                        ax_err!(NotConnected, "socket recv() failed")
                    } else if socket.can_recv() {
                        // data available
                        op(socket)
                    } else {
                        // no more data
                        Err(AxError::WouldBlock)
                    }
                })
                .await
        })
        .await
    }

    async fn block_on<F, T>(&self, mut f: F) -> AxResult<T>
    where
        F: AsyncFnMut() -> AxResult<T>,
    {
        if self.is_nonblocking() {
            f().await
        } else {
            loop {
                #[cfg(feature = "monolithic")]
                if executor::signal::current_have_signals().await {
                    return Err(AxError::Interrupted);
                }

                SOCKET_SET.poll_interfaces().await;
                match f().await {
                    Ok(t) => return Ok(t),
                    Err(AxError::WouldBlock) => executor::yield_now().await,
                    Err(e) => return Err(e),
                }
            }
        }
    }

    /// To get the socket and call the given function.
    ///
    /// If the socket is not connected, it will return None.
    ///
    /// Or it will return the result of the given function.
    pub async fn with_socket<R>(&self, f: impl FnOnce(&udp::Socket) -> R) -> R {
        SOCKET_SET.with_socket(self.handle, async |s| f(s)).await
    }
}

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

impl AsyncRead for UdpSocket {
    // fn read(&mut self, buf: &mut [u8]) -> AxResult<usize> {
    //     self.recv(buf)
    // }
    fn read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<AxResult<usize>> {
        Box::pin(self.recv(buf)).as_mut().poll(cx)
    }
}

impl AsyncWrite for UdpSocket {
    fn write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<AxResult<usize>> {
        Box::pin(self.send(buf)).as_mut().poll(cx)
    }
    fn flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        Poll::Ready(Err(AxError::Unsupported))
    }
    fn close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        Poll::Ready(Ok(()))
    }
}

// impl Write for UdpSocket {
//     fn write(&mut self, buf: &[u8]) -> AxResult<usize> {
//         self.send(buf)
//     }

//     fn flush(&mut self) -> AxResult {
//         Err(AxError::Unsupported)
//     }
// }

impl Drop for UdpSocket {
    fn drop(&mut self) {
        let waker = taskctx::CurrentTask::get().waker();
        let _ = Box::pin(async {
            self.shutdown().await.ok();
            SOCKET_SET.remove(self.handle).await;
        })
        .as_mut()
        .poll(&mut Context::from_waker(&waker));
        // self.shutdown().ok();
        // SOCKET_SET.remove(self.handle);
    }
}

async fn get_ephemeral_port() -> AxResult<u16> {
    const PORT_START: u16 = 0xc000;
    const PORT_END: u16 = 0xffff;
    static CURR: Mutex<u16> = Mutex::new(PORT_START);
    let mut curr = CURR.lock().await;

    let port = *curr;
    if *curr == PORT_END {
        *curr = PORT_START;
    } else {
        *curr += 1;
    }
    Ok(port)
}
