use alloc::{boxed::Box, collections::VecDeque};
use core::future::Future;
use core::ops::{Deref, DerefMut};

use axerrno::{ax_err, AxError, AxResult};
use smoltcp::iface::{SocketHandle, SocketSet};
use smoltcp::socket::tcp::{self, State};
use smoltcp::wire::{IpAddress, IpEndpoint, IpListenEndpoint};
use sync::Mutex;

use super::{SocketSetWrapper, LISTEN_QUEUE_SIZE, SOCKET_SET};

const PORT_NUM: usize = 65536;

struct ListenTableEntry {
    listen_endpoint: IpListenEndpoint,
    syn_queue: VecDeque<SocketHandle>,
}

impl ListenTableEntry {
    pub fn new(listen_endpoint: IpListenEndpoint) -> Self {
        Self {
            listen_endpoint,
            syn_queue: VecDeque::with_capacity(LISTEN_QUEUE_SIZE),
        }
    }

    #[inline]
    fn can_accept(&self, dst: IpAddress) -> bool {
        match self.listen_endpoint.addr {
            Some(addr) => addr == dst,
            None => true,
        }
    }
}

impl Drop for ListenTableEntry {
    fn drop(&mut self) {
        let waker = taskctx::CurrentTask::get().waker();
        let _ = Box::pin(async {
            for &handle in &self.syn_queue {
                SOCKET_SET.remove(handle).await;
            }
        })
        .as_mut()
        .poll(&mut core::task::Context::from_waker(&waker));
        // for &handle in &self.syn_queue {
        //     SOCKET_SET.remove(handle);
        // }
    }
}

pub struct ListenTable {
    tcp: Box<[Mutex<Option<Box<ListenTableEntry>>>]>,
}

impl ListenTable {
    pub fn new() -> Self {
        let tcp = unsafe {
            let mut buf = Box::new_uninit_slice(PORT_NUM);
            for i in 0..PORT_NUM {
                buf[i].write(Mutex::new(None));
            }
            buf.assume_init()
        };
        Self { tcp }
    }

    pub async fn can_listen(&self, port: u16) -> bool {
        self.tcp[port as usize].lock().await.is_none()
    }

    pub async fn listen(&self, listen_endpoint: IpListenEndpoint) -> AxResult {
        let port = listen_endpoint.port;
        assert_ne!(port, 0);
        let mut entry = self.tcp[port as usize].lock().await;
        if entry.is_none() {
            *entry = Some(Box::new(ListenTableEntry::new(listen_endpoint)));
            Ok(())
        } else {
            ax_err!(AddrInUse, "socket listen() failed")
        }
    }

    pub async fn unlisten(&self, port: u16) {
        debug!("TCP socket unlisten on {}", port);
        *self.tcp[port as usize].lock().await = None;
    }

    pub async fn can_accept(&self, port: u16) -> AxResult<bool> {
        if let Some(entry) = self.tcp[port as usize].lock().await.deref() {
            for &handle in &entry.syn_queue {
                if is_connected(handle).await {
                    return Ok(true);
                }
            }
            return Ok(false);
            // Ok(entry.syn_queue.iter().any(|&handle| is_connected(handle)))
        } else {
            ax_err!(InvalidInput, "socket accept() failed: not listen")
        }
    }

    pub async fn accept(&self, port: u16) -> AxResult<(SocketHandle, (IpEndpoint, IpEndpoint))> {
        if let Some(entry) = self.tcp[port as usize].lock().await.deref_mut() {
            let syn_queue: &mut VecDeque<SocketHandle> = &mut entry.syn_queue;
            let mut idx = -1;
            for (i, &handle) in syn_queue.iter().enumerate() {
                if is_connected(handle).await {
                    idx = i as isize;
                    break;
                }
            }
            if idx < 0 {
                return Err(AxError::WouldBlock);
            }
            // let idx = syn_queue
            //     .iter()
            //     .enumerate()
            //     .find_map(|(idx, &handle)| is_connected(handle).await.then(|| idx))
            //     .ok_or(AxError::WouldBlock)?; // wait for connection
            if idx > 0 {
                warn!(
                    "slow SYN queue enumeration: index = {}, len = {}!",
                    idx,
                    syn_queue.len()
                );
            }
            let handle = syn_queue.swap_remove_front(idx as usize).unwrap();
            // If the connection is reset, return ConnectionReset error
            // Otherwise, return the handle and the address tuple
            if is_closed(handle).await {
                ax_err!(ConnectionReset, "socket accept() failed: connection reset")
            } else {
                Ok((handle, get_addr_tuple(handle).await))
            }
        } else {
            ax_err!(InvalidInput, "socket accept() failed: not listen")
        }
    }

    pub async fn incoming_tcp_packet(
        &self,
        src: IpEndpoint,
        dst: IpEndpoint,
        sockets: &mut SocketSet<'_>,
    ) {
        if let Some(entry) = self.tcp[dst.port as usize].lock().await.deref_mut() {
            if !entry.can_accept(dst.addr) {
                // not listening on this address
                return;
            }
            if entry.syn_queue.len() >= LISTEN_QUEUE_SIZE {
                // SYN queue is full, drop the packet
                warn!("SYN queue overflow!");
                return;
            }
            let mut socket = SocketSetWrapper::new_tcp_socket();
            if socket.listen(entry.listen_endpoint).is_ok() {
                let handle = sockets.add(socket);
                debug!(
                    "TCP socket {}: prepare for connection {} -> {}",
                    handle, src, entry.listen_endpoint
                );
                entry.syn_queue.push_back(handle);
            }
        }
    }
}

async fn is_connected(handle: SocketHandle) -> bool {
    SOCKET_SET
        .with_socket::<tcp::Socket, _, _>(handle, async |socket| {
            !matches!(socket.state(), State::Listen | State::SynReceived)
        })
        .await
}

async fn is_closed(handle: SocketHandle) -> bool {
    SOCKET_SET
        .with_socket::<tcp::Socket, _, _>(handle, async |socket| {
            matches!(socket.state(), State::Closed)
        })
        .await
}

async fn get_addr_tuple(handle: SocketHandle) -> (IpEndpoint, IpEndpoint) {
    SOCKET_SET
        .with_socket::<tcp::Socket, _, _>(handle, async |socket| {
            (
                socket.local_endpoint().unwrap(),
                socket.remote_endpoint().unwrap(),
            )
        })
        .await
}
