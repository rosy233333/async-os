use core::cell::UnsafeCell;
use core::net::SocketAddr;
use core::ops::{AsyncFnMut, AsyncFnOnce};
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use alloc::boxed::Box;
use async_io::{AsyncRead, AsyncWrite, PollState};
use axerrno::{ax_err, ax_err_type, AxError, AxResult};
use axhal::time::current_ticks;
use sync::Mutex;

use process::yield_now;
use smoltcp::iface::SocketHandle;
use smoltcp::socket::tcp::{self, ConnectError, State};
use smoltcp::wire::{IpEndpoint, IpListenEndpoint};

use super::addr::{from_core_sockaddr, into_core_sockaddr, is_unspecified, UNSPECIFIED_ENDPOINT};
use super::{SocketSetWrapper, LISTEN_TABLE, SOCKET_SET};

// State transitions:
// CLOSED -(connect)-> BUSY -> CONNECTING -> CONNECTED -(shutdown)-> BUSY -> CLOSED
//       |
//       |-(listen)-> BUSY -> LISTENING -(shutdown)-> BUSY -> CLOSED
//       |
//        -(bind)-> BUSY -> CLOSED
const STATE_CLOSED: u8 = 0;
const STATE_BUSY: u8 = 1;
const STATE_CONNECTING: u8 = 2;
const STATE_CONNECTED: u8 = 3;
const STATE_LISTENING: u8 = 4;

/// A TCP socket that provides POSIX-like APIs.
///
/// - [`connect`] is for TCP clients.
/// - [`bind`], [`listen`], and [`accept`] are for TCP servers.
/// - Other methods are for both TCP clients and servers.
///
/// [`connect`]: TcpSocket::connect
/// [`bind`]: TcpSocket::bind
/// [`listen`]: TcpSocket::listen
/// [`accept`]: TcpSocket::accept
pub struct TcpSocket {
    state: AtomicU8,
    handle: UnsafeCell<Option<SocketHandle>>,
    local_addr: UnsafeCell<IpEndpoint>,
    peer_addr: UnsafeCell<IpEndpoint>,
    nonblock: AtomicBool,
    reuse_addr: AtomicBool,
}

unsafe impl Sync for TcpSocket {}

impl TcpSocket {
    /// Creates a new TCP socket.
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_CLOSED),
            handle: UnsafeCell::new(None),
            local_addr: UnsafeCell::new(UNSPECIFIED_ENDPOINT),
            peer_addr: UnsafeCell::new(UNSPECIFIED_ENDPOINT),
            nonblock: AtomicBool::new(false),
            reuse_addr: AtomicBool::new(false),
        }
    }

    /// Creates a new TCP socket that is already connected.
    const fn new_connected(
        handle: SocketHandle,
        local_addr: IpEndpoint,
        peer_addr: IpEndpoint,
    ) -> Self {
        Self {
            state: AtomicU8::new(STATE_CONNECTED),
            handle: UnsafeCell::new(Some(handle)),
            local_addr: UnsafeCell::new(local_addr),
            peer_addr: UnsafeCell::new(peer_addr),
            nonblock: AtomicBool::new(false),
            reuse_addr: AtomicBool::new(false),
        }
    }

    /// Returns the local address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    #[inline]
    pub fn local_addr(&self) -> AxResult<SocketAddr> {
        // 为了通过测例，已经`bind`但未`listen`的socket也可以返回地址
        match self.get_state() {
            STATE_CONNECTED | STATE_LISTENING | STATE_CLOSED => {
                Ok(into_core_sockaddr(unsafe { self.local_addr.get().read() }))
            }
            _ => Err(AxError::NotConnected),
        }
    }

    /// Returns the remote address and port, or
    /// [`Err(NotConnected)`](AxError::NotConnected) if not connected.
    #[inline]
    pub fn peer_addr(&self) -> AxResult<SocketAddr> {
        match self.get_state() {
            STATE_CONNECTED | STATE_LISTENING => {
                Ok(into_core_sockaddr(unsafe { self.peer_addr.get().read() }))
            }
            _ => Err(AxError::NotConnected),
        }
    }

    /// Returns whether this socket is in nonblocking mode.
    #[inline]
    pub fn is_nonblocking(&self) -> bool {
        self.nonblock.load(Ordering::Acquire)
    }

    /// Moves this TCP stream into or out of nonblocking mode.
    ///
    /// This will result in `read`, `write`, `recv` and `send` operations
    /// becoming nonblocking, i.e., immediately returning from their calls.
    /// If the IO operation is successful, `Ok` is returned and no further
    /// action is required. If the IO operation could not be completed and needs
    /// to be retried, an error with kind  [`Err(WouldBlock)`](AxError::WouldBlock) is
    /// returned.
    #[inline]
    pub fn set_nonblocking(&self, nonblocking: bool) {
        self.nonblock.store(nonblocking, Ordering::Release);
    }

    ///Returns whether this socket is in reuse address mode.
    #[inline]
    pub fn is_reuse_addr(&self) -> bool {
        self.reuse_addr.load(Ordering::Acquire)
    }

    /// Moves this TCP socket into or out of reuse address mode.
    ///
    /// When a socket is bound, the `SO_REUSEADDR` option allows multiple sockets to be bound to the
    /// same address if they are bound to different local addresses. This option must be set before
    /// calling `bind`.
    #[inline]
    pub fn set_reuse_addr(&self, reuse_addr: bool) {
        self.reuse_addr.store(reuse_addr, Ordering::Release);
    }

    /// To get the address pair of the socket.
    ///
    /// Returns the local and remote endpoint pair.
    // fn get_endpoint_pair(
    //     &self,
    //     remote_addr: SocketAddr,
    // ) -> Result<(IpListenEndpoint, IpEndpoint), AxError> {
    //     // TODO: check remote addr unreachable
    //     #[allow(unused_mut)]
    //     let mut remote_endpoint = from_core_sockaddr(remote_addr);
    //     #[allow(unused_mut)]
    //     let mut bound_endpoint = self.bound_endpoint()?;
    //     // #[cfg(feature = "ip")]
    //     if bound_endpoint.addr.is_none() && remote_endpoint.addr.as_bytes()[0] == 127 {
    //         // If the remote addr is unspecified, we should copy the local addr.
    //         // If the local addr is unspecified too, we should use the loopback interface.
    //         if remote_endpoint.addr.is_unspecified() {
    //             remote_endpoint.addr =
    //                 smoltcp::wire::IpAddress::Ipv4(smoltcp::wire::Ipv4Address::new(127, 0, 0, 1));
    //         }
    //         bound_endpoint.addr = Some(remote_endpoint.addr);
    //     }
    //     Ok((bound_endpoint, remote_endpoint))
    // }

    /// Connects to the given address and port.
    ///
    /// The local port is generated automatically.
    pub async fn connect(&self, remote_addr: SocketAddr) -> AxResult {
        self.update_state(STATE_CLOSED, STATE_CONNECTING, async || {
            // SAFETY: no other threads can read or write these fields.
            // let handle = unsafe { self.handle.get().read() }
            //     .unwrap_or_else(|| SOCKET_SET.add(SocketSetWrapper::new_tcp_socket()));
            let handle = if let Some(h) = unsafe { self.handle.get().read() } {
                h
            } else {
                SOCKET_SET.add(SocketSetWrapper::new_tcp_socket()).await
            };
            // // TODO: check remote addr unreachable
            // let (bound_endpoint, remote_endpoint) = self.get_endpoint_pair(remote_addr)?;
            let remote_endpoint = from_core_sockaddr(remote_addr);
            let bound_endpoint = self.bound_endpoint().await?;
            info!("bound endpoint: {:?}", bound_endpoint);
            info!("remote endpoint: {:?}", remote_endpoint);
            warn!("Temporarily net bridge used");
            let iface = if remote_endpoint.addr.as_bytes()[0] == 127 {
                super::LOOPBACK.try_get().unwrap()
            } else {
                info!("Use eth net");
                &super::ETH0.iface
            };

            let (local_endpoint, remote_endpoint) = SOCKET_SET
                .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| {
                    socket
                        .connect(
                            iface.lock().await.context(),
                            remote_endpoint,
                            bound_endpoint,
                        )
                        .or_else(|e| match e {
                            ConnectError::InvalidState => {
                                ax_err!(BadState, "socket connect() failed")
                            }
                            ConnectError::Unaddressable => {
                                ax_err!(ConnectionRefused, "socket connect() failed")
                            }
                        })?;
                    Ok::<(IpEndpoint, IpEndpoint), AxError>((
                        socket.local_endpoint().unwrap(),
                        socket.remote_endpoint().unwrap(),
                    ))
                })
                .await?;
            unsafe {
                // SAFETY: no other threads can read or write these fields as we
                // have changed the state to `BUSY`.
                self.local_addr.get().write(local_endpoint);
                self.peer_addr.get().write(remote_endpoint);
                self.handle.get().write(Some(handle));
            }
            Ok(())
        })
        .await
        .unwrap_or_else(|_| ax_err!(AlreadyExists, "socket connect() failed: already connected"))?; // EISCONN

        // HACK: yield() to let server to listen
        yield_now();

        // Here our state must be `CONNECTING`, and only one thread can run here.
        if self.is_nonblocking() {
            Err(AxError::WouldBlock)
        } else {
            self.block_on(async || {
                let PollState { writable, .. } = self.poll_connect().await?;
                if !writable {
                    Err(AxError::WouldBlock)
                } else if self.get_state() == STATE_CONNECTED {
                    Ok(())
                } else {
                    ax_err!(ConnectionRefused, "socket connect() failed")
                }
            })
            .await
        }
    }

    /// Binds an unbound socket to the given address and port.
    ///
    /// If the given port is 0, it generates one automatically.
    ///
    /// It's must be called before [`listen`](Self::listen) and
    /// [`accept`](Self::accept).
    pub async fn bind(&self, mut local_addr: SocketAddr) -> AxResult {
        self.update_state(STATE_CLOSED, STATE_CLOSED, async || {
            // TODO: check addr is available
            if local_addr.port() == 0 {
                local_addr.set_port(get_ephemeral_port().await?);
            }
            // SAFETY: no other threads can read or write `self.local_addr` as we
            // have changed the state to `BUSY`.
            unsafe {
                let old = self.local_addr.get().read();
                if old != UNSPECIFIED_ENDPOINT {
                    return ax_err!(InvalidInput, "socket bind() failed: already bound");
                }
                self.local_addr.get().write(from_core_sockaddr(local_addr));
            }
            let local_endpoint = from_core_sockaddr(local_addr);
            let bound_endpoint = self.bound_endpoint().await?;
            // let handle = unsafe { self.handle.get().read() }
            //     .unwrap_or_else(|| SOCKET_SET.add(SocketSetWrapper::new_tcp_socket()));
            let handle = if let Some(h) = unsafe { self.handle.get().read() } {
                h
            } else {
                SOCKET_SET.add(SocketSetWrapper::new_tcp_socket()).await
            };
            SOCKET_SET
                .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| {
                    socket.set_bound_endpoint(bound_endpoint);
                })
                .await;

            if !self.is_reuse_addr() {
                SOCKET_SET
                    .bind_check(local_endpoint.addr, local_endpoint.port)
                    .await?;
            }
            Ok(())
        })
        .await
        .unwrap_or_else(|_| ax_err!(InvalidInput, "socket bind() failed: already bound"))
    }

    /// Starts listening on the bound address and port.
    ///
    /// It's must be called after [`bind`](Self::bind) and before
    /// [`accept`](Self::accept).
    pub async fn listen(&self) -> AxResult {
        self.update_state(STATE_CLOSED, STATE_LISTENING, async || {
            let bound_endpoint = self.bound_endpoint().await?;
            unsafe {
                (*self.local_addr.get()).port = bound_endpoint.port;
            }
            LISTEN_TABLE.listen(bound_endpoint).await?;
            debug!("TCP socket listening on {}", bound_endpoint);
            Ok(())
        })
        .await
        .unwrap_or(Ok(())) // ignore simultaneous `listen`s.
    }

    /// Accepts a new connection.
    ///
    /// This function will block the calling thread until a new TCP connection
    /// is established. When established, a new [`TcpSocket`] is returned.
    ///
    /// It's must be called after [`bind`](Self::bind) and [`listen`](Self::listen).
    pub async fn accept(&self) -> AxResult<TcpSocket> {
        if !self.is_listening() {
            return ax_err!(InvalidInput, "socket accept() failed: not listen");
        }

        // SAFETY: `self.local_addr` should be initialized after `bind()`.
        let local_port = unsafe { self.local_addr.get().read().port };
        self.block_on(async || {
            let (handle, (local_addr, peer_addr)) = LISTEN_TABLE.accept(local_port).await?;
            debug!("TCP socket accepted a new connection {}", peer_addr);
            Ok(TcpSocket::new_connected(handle, local_addr, peer_addr))
        })
        .await
    }

    /// Close the connection.
    pub async fn shutdown(&self) -> AxResult {
        // stream
        self.update_state(STATE_CONNECTED, STATE_CLOSED, async || {
            // SAFETY: `self.handle` should be initialized in a connected socket, and
            // no other threads can read or write it.
            let handle = unsafe { self.handle.get().read().unwrap() };
            SOCKET_SET
                .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| {
                    debug!("TCP socket {}: shutting down", handle);
                    socket.close();
                })
                .await;
            unsafe { self.local_addr.get().write(UNSPECIFIED_ENDPOINT) }; // clear bound address
            SOCKET_SET.poll_interfaces().await;
            Ok(())
        })
        .await
        .unwrap_or(Ok(()))?;

        // listener
        self.update_state(STATE_LISTENING, STATE_CLOSED, async || {
            // SAFETY: `self.local_addr` should be initialized in a listening socket,
            // and no other threads can read or write it.
            let local_port = unsafe { self.local_addr.get().read().port };
            unsafe { self.local_addr.get().write(UNSPECIFIED_ENDPOINT) }; // clear bound address
            LISTEN_TABLE.unlisten(local_port).await;
            SOCKET_SET.poll_interfaces().await;
            Ok(())
        })
        .await
        .unwrap_or(Ok(()))?;

        // ignore for other states
        Ok(())
    }

    /// Close the transmit half of the tcp socket.
    /// It will call `close()` on smoltcp::socket::tcp::Socket. It should send FIN to remote half.
    ///
    /// This function is for shutdown(fd, SHUT_WR) syscall.
    ///
    /// It won't change TCP state.
    /// It won't affect unconnected sockets (listener).
    pub async fn close(&self) {
        let handle = match unsafe { self.handle.get().read() } {
            Some(h) => h,
            None => return,
        };
        SOCKET_SET
            .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| socket.close())
            .await;
        SOCKET_SET.poll_interfaces().await;
    }

    /// Receives data from the socket, stores it in the given buffer.
    pub async fn recv(&self, buf: &mut [u8]) -> AxResult<usize> {
        if self.is_connecting() {
            return Err(AxError::WouldBlock);
        } else if !self.is_connected() {
            return ax_err!(NotConnected, "socket recv() failed");
        }

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        self.block_on(async || {
            SOCKET_SET
                .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| {
                    if socket.recv_queue() > 0 {
                        // data available
                        // TODO: use socket.recv(|buf| {...})
                        let len = socket
                            .recv_slice(buf)
                            .map_err(|_| ax_err_type!(BadState, "socket recv() failed"))?;
                        Ok(len)
                    } else if !socket.is_active() {
                        // not open
                        ax_err!(ConnectionRefused, "socket recv() failed")
                    } else if !socket.may_recv() {
                        // connection closed
                        Ok(0)
                    } else {
                        // no more data
                        Err(AxError::WouldBlock)
                    }
                })
                .await
        })
        .await
    }
    /// Receives data from the socket, stores it in the given buffer.
    ///
    /// It will return [`Err(Timeout)`](AxError::Timeout) if expired.
    pub async fn recv_timeout(&self, buf: &mut [u8], ticks: u64) -> AxResult<usize> {
        if self.is_connecting() {
            return Err(AxError::WouldBlock);
        } else if !self.is_connected() {
            return ax_err!(NotConnected, "socket recv() failed");
        }

        let expire_at = current_ticks() + ticks;

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        self.block_on(async || {
            SOCKET_SET
                .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| {
                    if socket.recv_queue() > 0 {
                        // data available
                        // TODO: use socket.recv(|buf| {...})
                        let len = socket
                            .recv_slice(buf)
                            .map_err(|_| ax_err_type!(BadState, "socket recv() failed"))?;
                        Ok(len)
                    } else if !socket.is_active() {
                        // not open
                        ax_err!(ConnectionRefused, "socket recv() failed")
                    } else if !socket.may_recv() {
                        // connection closed
                        Ok(0)
                    } else {
                        // no more data
                        if current_ticks() > expire_at {
                            Err(AxError::Timeout)
                        } else {
                            Err(AxError::WouldBlock)
                        }
                    }
                })
                .await
        })
        .await
    }

    /// Transmits data in the given buffer.
    pub async fn send(&self, buf: &[u8]) -> AxResult<usize> {
        if self.is_connecting() {
            return Err(AxError::WouldBlock);
        } else if !self.is_connected() {
            return ax_err!(NotConnected, "socket send() failed");
        }

        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        self.block_on(async || {
            SOCKET_SET
                .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| {
                    if !socket.is_active() || !socket.may_send() {
                        // closed by remote
                        ax_err!(ConnectionReset, "socket send() failed")
                    } else if socket.can_send() {
                        // connected, and the tx buffer is not full
                        // TODO: use socket.send(|buf| {...})
                        let len = socket
                            .send_slice(buf)
                            .map_err(|_| ax_err_type!(BadState, "socket send() failed"))?;
                        Ok(len)
                    } else {
                        // tx buffer is full
                        Err(AxError::WouldBlock)
                    }
                })
                .await
        })
        .await
    }

    /// Whether the socket is readable or writable.
    pub async fn poll(&self) -> AxResult<PollState> {
        match self.get_state() {
            STATE_CONNECTING => self.poll_connect().await,
            STATE_CONNECTED => self.poll_stream().await,
            STATE_LISTENING => self.poll_listener().await,
            _ => Ok(PollState {
                readable: false,
                writable: false,
            }),
        }
    }

    /// To set the nagle algorithm enabled or not.
    pub async fn set_nagle_enabled(&self, enabled: bool) -> AxResult {
        let handle = unsafe { self.handle.get().read() };

        let Some(handle) = handle else {
            return Err(AxError::NotConnected);
        };

        SOCKET_SET
            .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| {
                socket.set_nagle_enabled(enabled)
            })
            .await;

        Ok(())
    }

    /// To get the nagle algorithm enabled or not.
    pub async fn nagle_enabled(&self) -> bool {
        let handle = unsafe { self.handle.get().read() };

        match handle {
            Some(handle) => {
                SOCKET_SET
                    .with_socket::<tcp::Socket, _, _>(handle, async |socket| socket.nagle_enabled())
                    .await
            }
            // Nagle algorithm will be enabled by default once the socket is created
            None => true,
        }
    }

    /// To get the socket and call the given function.
    ///
    /// If the socket is not connected, it will return None.
    ///
    /// Or it will return the result of the given function.
    pub async fn with_socket<R>(&self, f: impl AsyncFnOnce(Option<&tcp::Socket>) -> R) -> R {
        let handle = unsafe { self.handle.get().read() };

        match handle {
            Some(handle) => {
                SOCKET_SET
                    .with_socket::<tcp::Socket, _, _>(handle, async |socket| f(Some(socket)).await)
                    .await
            }
            None => f(None).await,
        }
    }

    /// To get the mutable socket and call the given function.
    ///
    /// If the socket is not connected, it will return None.
    ///
    /// Or it will return the result of the given function.
    pub async fn with_socket_mut<R>(
        &self,
        f: impl AsyncFnOnce(Option<&mut tcp::Socket>) -> R,
    ) -> R {
        let handle = unsafe { self.handle.get().read() };

        match handle {
            Some(handle) => {
                SOCKET_SET
                    .with_socket_mut::<tcp::Socket, _, _>(handle, async |socket| {
                        f(Some(socket)).await
                    })
                    .await
            }
            None => f(None).await,
        }
    }
}

/// Private methods
impl TcpSocket {
    #[inline]
    fn get_state(&self) -> u8 {
        self.state.load(Ordering::Acquire)
    }

    #[inline]
    fn set_state(&self, state: u8) {
        self.state.store(state, Ordering::Release);
    }

    /// Update the state of the socket atomically.
    ///
    /// If the current state is `expect`, it first changes the state to `STATE_BUSY`,
    /// then calls the given function. If the function returns `Ok`, it changes the
    /// state to `new`, otherwise it changes the state back to `expect`.
    ///
    /// It returns `Ok` if the current state is `expect`, otherwise it returns
    /// the current state in `Err`.
    async fn update_state<F, T>(&self, expect: u8, new: u8, f: F) -> Result<AxResult<T>, u8>
    where
        F: AsyncFnOnce() -> AxResult<T>,
    {
        match self
            .state
            .compare_exchange(expect, STATE_BUSY, Ordering::Acquire, Ordering::Acquire)
        {
            Ok(_) => {
                let res = f().await;
                if res.is_ok() {
                    self.set_state(new);
                } else {
                    self.set_state(expect);
                }
                Ok(res)
            }
            Err(old) => Err(old),
        }
    }

    #[inline]
    fn is_connecting(&self) -> bool {
        self.get_state() == STATE_CONNECTING
    }

    #[inline]
    /// Whether the socket is connected.
    pub fn is_connected(&self) -> bool {
        self.get_state() == STATE_CONNECTED
    }

    #[inline]
    /// Whether the socket is closed.
    pub fn is_closed(&self) -> bool {
        self.get_state() == STATE_CLOSED
    }

    #[inline]
    fn is_listening(&self) -> bool {
        self.get_state() == STATE_LISTENING
    }

    async fn bound_endpoint(&self) -> AxResult<IpListenEndpoint> {
        // SAFETY: no other threads can read or write `self.local_addr`.
        let local_addr = unsafe { self.local_addr.get().read() };
        let port = if local_addr.port != 0 {
            local_addr.port
        } else {
            get_ephemeral_port().await?
        };
        assert_ne!(port, 0);
        let addr = if !is_unspecified(local_addr.addr) {
            Some(local_addr.addr)
        } else {
            None
        };
        Ok(IpListenEndpoint { addr, port })
    }

    async fn poll_connect(&self) -> AxResult<PollState> {
        // SAFETY: `self.handle` should be initialized above.
        let handle = unsafe { self.handle.get().read().unwrap() };
        let writable = SOCKET_SET
            .with_socket::<tcp::Socket, _, _>(handle, async |socket| {
                match socket.state() {
                    State::SynSent => false, // wait for connection
                    State::Established => {
                        self.set_state(STATE_CONNECTED); // connected
                        debug!(
                            "TCP socket {}: connected to {}",
                            handle,
                            socket.remote_endpoint().unwrap(),
                        );
                        true
                    }
                    _ => {
                        unsafe {
                            self.local_addr.get().write(UNSPECIFIED_ENDPOINT);
                            self.peer_addr.get().write(UNSPECIFIED_ENDPOINT);
                        }
                        self.set_state(STATE_CLOSED); // connection failed
                        true
                    }
                }
            })
            .await;
        Ok(PollState {
            readable: false,
            writable,
        })
    }

    async fn poll_stream(&self) -> AxResult<PollState> {
        // SAFETY: `self.handle` should be initialized in a connected socket.
        let handle = unsafe { self.handle.get().read().unwrap() };
        SOCKET_SET
            .with_socket::<tcp::Socket, _, _>(handle, async |socket| {
                Ok(PollState {
                    readable: !socket.may_recv() || socket.can_recv(),
                    writable: !socket.may_send() || socket.can_send(),
                })
            })
            .await
    }

    async fn poll_listener(&self) -> AxResult<PollState> {
        // SAFETY: `self.local_addr` should be initialized in a listening socket.
        let local_addr = unsafe { self.local_addr.get().read() };
        Ok(PollState {
            readable: LISTEN_TABLE.can_accept(local_addr.port).await?,
            writable: false,
        })
    }

    /// Block the current thread until the given function completes or fails.
    ///
    /// If the socket is non-blocking, it calls the function once and returns
    /// immediately. Otherwise, it may call the function multiple times if it
    /// returns [`Err(WouldBlock)`](AxError::WouldBlock).
    async fn block_on<F, T>(&self, mut f: F) -> AxResult<T>
    where
        F: AsyncFnMut() -> AxResult<T>,
    {
        if self.is_nonblocking() {
            f().await
        } else {
            loop {
                #[cfg(feature = "monolithic")]
                if process::signal::current_have_signals().await {
                    return Err(AxError::Interrupted);
                }

                SOCKET_SET.poll_interfaces().await;
                match f().await {
                    Ok(t) => return Ok(t),
                    Err(AxError::WouldBlock) => process::yield_now().await,
                    Err(e) => return Err(e),
                }
            }
        }
    }
}

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

impl AsyncRead for TcpSocket {
    fn read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<AxResult<usize>> {
        Box::pin(self.recv(buf)).as_mut().poll(cx)
    }
}

impl AsyncWrite for TcpSocket {
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

impl Drop for TcpSocket {
    fn drop(&mut self) {
        let waker = taskctx::CurrentTask::get().waker();
        let _ = Box::pin(async {
            self.shutdown().await.ok();
            // Safe because we have mut reference to `self`.
            if let Some(handle) = unsafe { self.handle.get().read() } {
                SOCKET_SET.remove(handle).await;
            }
        })
        .as_mut()
        .poll(&mut Context::from_waker(&waker));
        // self.shutdown().ok();
        // // Safe because we have mut reference to `self`.
        // if let Some(handle) = unsafe { self.handle.get().read() } {
        //     SOCKET_SET.remove(handle);
        // }
    }
}

async fn get_ephemeral_port() -> AxResult<u16> {
    const PORT_START: u16 = 0xc000;
    const PORT_END: u16 = 0xffff;
    static CURR: Mutex<u16> = Mutex::new(PORT_START);

    let mut curr = CURR.lock().await;
    let mut tries = 0;
    // TODO: more robust
    while tries <= PORT_END - PORT_START {
        let port = *curr;
        if *curr == PORT_END {
            *curr = PORT_START;
        } else {
            *curr += 1;
        }
        if LISTEN_TABLE.can_listen(port).await {
            return Ok(port);
        }
        tries += 1;
    }
    ax_err!(AddrInUse, "no avaliable ports!")
}
