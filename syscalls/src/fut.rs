use crate::{raw, Errno, Sysno};
use core::{pin::Pin, task::{Context, Poll}, future::Future};

pub struct SyscallFuture {
    pub id: Sysno,
    pub args: Vec<usize>,
    pub res: Option<Result<usize, Errno>>,
}

impl SyscallFuture {

    pub fn new(id: Sysno, args: &[usize]) -> Self {
        Self { id, args: Vec::from(args), res: None }
    }

    fn run(&mut self) {
        // 目前仍然是通过 ecall 来发起系统调用
        const ASYNC_FLAG: usize = 0x5f5f5f5f;
        let res = unsafe { match self.args.len() {
            0 => raw::raw_syscall!(self.id as _, ASYNC_FLAG),
            1 => raw::raw_syscall!(self.id as _, self.args[0], ASYNC_FLAG),
            2 => raw::raw_syscall!(self.id as _, self.args[0], self.args[1], ASYNC_FLAG),
            3 => raw::raw_syscall!(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2],
                ASYNC_FLAG
            ),
            4 => raw::raw_syscall!(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3],
                ASYNC_FLAG
            ),
            5 => raw::raw_syscall!(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4],
                ASYNC_FLAG
            ),
            6 => raw::raw_syscall!(
                self.id as _, 
                self.args[0], 
                self.args[1], 
                self.args[2], 
                self.args[3], 
                self.args[4], 
                self.args[5]
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
        if let Some(ret) = this.res {
            return Poll::Ready(ret);
        } else {
            this.run();
            if let Some(ret) = this.res {
                return Poll::Ready(ret);
            } else {
                return Poll::Pending;
            }
        }
    }
}