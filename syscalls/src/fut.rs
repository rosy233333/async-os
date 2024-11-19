use crate::{raw, Errno, Sysno};
use core::{
    cell::Cell, future::Future, ops::Deref, pin::Pin, task::{Context, Poll}
};
use alloc::{boxed::Box, vec::Vec};

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
    pub id: Sysno,
    pub args: Vec<usize>,
    pub res: SyscallRes,
}

impl SyscallFuture {

    pub fn new(id: Sysno, args: &[usize]) -> Self {
        Self { has_issued: false, id, args: Vec::from(args), res: SyscallRes(Box::pin(Cell::new(None))) }
    }

    pub(crate) fn run(&mut self) {
        // 目前仍然是通过 ecall 来发起系统调用
        let _ret_ptr = self.res.get_ptr() as *mut usize as usize;
        // 需要新增一个参数来记录返回值的位置
        // 详细的设置见 crate::raw_syscall
        #[cfg(target_arch = "riscv64")]
        let res = unsafe { match self.args.len() {
            0 => raw::syscall0(self.id as _, _ret_ptr),
            1 => raw::syscall1(self.id as _, self.args[0], _ret_ptr),
            2 => raw::syscall2(self.id as _, self.args[0], self.args[1], _ret_ptr),
            3 => raw::syscall3(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2],
                _ret_ptr
            ),
            4 => raw::syscall4(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3],
                _ret_ptr
            ),
            5 => raw::syscall5(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4],
                _ret_ptr
            ),
            6 => raw::syscall6(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4], 
                self.args[5],
                _ret_ptr
            ),
            _ => panic!("not support the number of syscall args > 6"),
        }};
        #[cfg(not(target_arch = "riscv64"))]
        let res = unsafe { match self.args.len() {
            0 => raw::syscall0(self.id as _),
            1 => raw::syscall1(self.id as _, self.args[0]),
            2 => raw::syscall2(self.id as _, self.args[0], self.args[1]),
            3 => raw::syscall3(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2],
            ),
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
        }};
        if res as i32 != Errno::EAGAIN.into_raw()  {
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
                this.run();
            }
            if let Some(ret) = this.res.get() {
                return Poll::Ready(ret);
            } else {
                #[cfg(feature = "yield-pending")]
                {
                    // 设置任务状态，使协程返回Pending后视为yield而非wait。
                    let mut yield_fut = Box::pin(user_task_scheduler::yield_now());
                    yield_fut.as_mut().poll(cx);
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
        unsafe {
            &(&*self.res.0.as_ptr()).as_ref().unwrap()
        }
    }
}