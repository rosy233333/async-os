use crate::{raw, AsyncFlags, Errno, Sysno, TaskOps};
use alloc::{boxed::Box, vec::Vec};
use core::{
    cell::Cell,
    future::Future,
    ops::Deref,
    pin::Pin,
    task::{Context, Poll, Waker},
};

#[repr(C)]
pub struct SyscallRes(Pin<Box<Cell<Option<Result<usize, Errno>>>>>);

impl SyscallRes {
    pub fn get_ptr(&mut self) -> *mut Option<Result<usize, Errno>> {
        self.0.as_ptr()
    }

    pub fn replace(&mut self, res: Result<usize, Errno>) {
        (*self.0).set(Some(res));
    }

    pub fn get(&self) -> Option<Result<usize, Errno>> {
        self.0.get()
    }
}

pub struct SyscallFuture {
    pub has_issued: bool,
    pub id: usize,
    pub args: Vec<usize>,
    pub res: SyscallRes,
}

impl SyscallFuture {
    pub fn new(id: usize, args: &[usize]) -> Self {
        Self {
            has_issued: false,
            id,
            args: Vec::from(args),
            res: SyscallRes(Box::pin(Cell::new(None))),
        }
    }

    pub fn get_ret_ptr(&mut self) -> usize {
        self.res.get_ptr() as *mut usize as usize
    }

    pub fn get_args(&self) -> [usize; 6] {
        let mut args = [0usize; 6];
        for (idx, arg) in self.args.iter().enumerate() {
            args[idx] = *arg;
        }
        args
    }

    pub(crate) fn run(&mut self, flag: AsyncFlags, waker: Option<&Waker>) {
        // 目前仍然是通过 ecall 来发起系统调用
        let _ret_ptr = match flag {
            AsyncFlags::ASYNC => Some((self.res.get_ptr() as *mut usize as usize, waker.unwrap())),
            AsyncFlags::SYNC => None,
        };
        // 需要新增一个参数来记录返回值的位置
        // 详细的设置见 crate::raw_syscall
        #[cfg(target_arch = "riscv64")]
        let res = unsafe {
            match self.args.len() {
                0 => raw::syscall0(self.id as _, _ret_ptr),
                1 => raw::syscall1(self.id as _, self.args[0], _ret_ptr),
                2 => raw::syscall2(self.id as _, self.args[0], self.args[1], _ret_ptr),
                3 => raw::syscall3(
                    self.id as _,
                    self.args[0],
                    self.args[1],
                    self.args[2],
                    _ret_ptr,
                ),
                4 => raw::syscall4(
                    self.id as _,
                    self.args[0],
                    self.args[1],
                    self.args[2],
                    self.args[3],
                    _ret_ptr,
                ),
                5 => raw::syscall5(
                    self.id as _,
                    self.args[0],
                    self.args[1],
                    self.args[2],
                    self.args[3],
                    self.args[4],
                    _ret_ptr,
                ),
                6 => raw::syscall6(
                    self.id as _,
                    self.args[0],
                    self.args[1],
                    self.args[2],
                    self.args[3],
                    self.args[4],
                    self.args[5],
                    _ret_ptr,
                ),
                _ => panic!("not support the number of syscall args > 6"),
            }
        };
        #[cfg(not(target_arch = "riscv64"))]
        let res = unsafe {
            match self.args.len() {
                0 => raw::syscall0(self.id as _),
                1 => raw::syscall1(self.id as _, self.args[0]),
                2 => raw::syscall2(self.id as _, self.args[0], self.args[1]),
                3 => raw::syscall3(self.id as _, self.args[0], self.args[1], self.args[2]),
                4 => raw::syscall4(
                    self.id as _,
                    self.args[0],
                    self.args[1],
                    self.args[2],
                    self.args[3],
                ),
                5 => raw::syscall5(
                    self.id as _,
                    self.args[0],
                    self.args[1],
                    self.args[2],
                    self.args[3],
                    self.args[4],
                ),
                6 => raw::syscall6(
                    self.id as _,
                    self.args[0],
                    self.args[1],
                    self.args[2],
                    self.args[3],
                    self.args[4],
                    self.args[5],
                ),
                _ => panic!("not support the number of syscall args > 6"),
            }
        };
        if res as i32 != Errno::EAGAIN.into_raw() {
            self.res.replace(Errno::from_ret(res));
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.res.get().is_some()
    }
}

impl Future for SyscallFuture {
    type Output = Result<usize, Errno>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if let Some(ret) = this.res.get() {
            return Poll::Ready(ret);
        } else {
            if !this.has_issued {
                this.has_issued = true;
                #[cfg(feature = "blocking")]
                this.run(AsyncFlags::SYNC, None);
                #[cfg(not(feature = "blocking"))]
                this.run(AsyncFlags::ASYNC, Some(cx.waker()));
            }
            if let Some(ret) = this.res.get() {
                return Poll::Ready(ret);
            } else {
                #[cfg(feature = "yield-pending")]
                {
                    use crate::task_trait::__TaskOps_mod;
                    crate_interface::call_interface!(TaskOps::set_state_yield());
                }
                return Poll::Pending;
            }
        }
    }
}

impl Deref for SyscallFuture {
    type Target = Result<usize, Errno>;

    /// 对于non-await的调用方式，无论non-blocking还是blocking，都可以直接调用该函数获取结果。非阻塞调用可能出现的让出在系统调用API内部进行。
    /// 对于await的调用方式，不应使用deref函数，而应使用.await。使用该函数会通不过assert。
    fn deref(&self) -> &Self::Target {
        assert!(self.has_issued);
        unsafe { &(&*self.res.0.as_ptr()).as_ref().unwrap() }
    }
}
