use crate::{raw, Errno, Sysno};
use core::{
    pin::Pin, 
    task::{Context, Poll}, 
    future::Future, 
    cell::Cell,
};

pub(crate) const ASYNC_FLAG: usize = 0x5f5f5f5f;

#[repr(C)]
pub struct SyscallRes(Cell<Option<Result<usize, Errno>>>);

impl SyscallRes {

    pub fn get_ptr(&mut self) -> *mut Option<Result<usize, Errno>> {
        self.0.as_ptr()
    }

    pub fn replace(&mut self, res: Result<usize, Errno>) {
        self.0.set(Some(res));
    }

    pub fn get(&mut self) -> Option<Result<usize, Errno>> {
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
        Self { has_issued: false, id, args: Vec::from(args), res: SyscallRes(Cell::new(None)) }
    }

    fn run(&mut self) {
        // 目前仍然是通过 ecall 来发起系统调用
        let ret_ptr = self.res.get_ptr() as *mut usize as usize;
        // 需要新增一个参数来记录返回值的位置
        // 详细的设置见 crate::raw_syscall
        let res = unsafe { match self.args.len() {
            0 => raw::syscall0(self.id as _, ret_ptr),
            1 => raw::syscall1(self.id as _, self.args[0], ret_ptr),
            2 => raw::syscall2(self.id as _, self.args[0], self.args[1], ret_ptr),
            3 => raw::syscall3(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2],
                ret_ptr
            ),
            4 => raw::syscall4(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3],
                ret_ptr
            ),
            5 => raw::syscall5(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4],
                ret_ptr
            ),
            6 => raw::syscall6(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4], 
                self.args[5],
                ret_ptr
            ),
            _ => panic!("not support the number of syscall args > 6"),
        }};
        if res as i32 != Errno::EAGAIN.into_raw()  {
            self.res.replace(Errno::from_ret(res));
        }
    }
}

impl Future for SyscallFuture {
    type Output = Result<usize, Errno>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
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
                return Poll::Pending;
            }
        }
    }
}