use core::mem::ManuallyDrop;

use alloc::sync::Arc;
use taskctx::{Task, TaskInner, TaskState};

use crate::restore_from_stack_ctx;

impl libvsched2::Task for Task {
    #[doc = r" 任务状态"]
    fn state(&self) -> libvsched2::TaskState {
        let state = TaskInner::state(self);
        match state {
            TaskState::Running => libvsched2::TaskState::Running,
            TaskState::Runable => libvsched2::TaskState::Ready,
            TaskState::Waked => libvsched2::TaskState::Running,
            TaskState::Blocked => libvsched2::TaskState::Blocked,
            TaskState::Blocking => libvsched2::TaskState::Running,
            TaskState::Exited => libvsched2::TaskState::Exited,
        }
    }

    #[doc = r" 设置任务状态"]
    fn set_state(&self, state: libvsched2::TaskState) -> libvsched2::TaskState {
        let prev_state = libvsched2::Task::state(self);
        let curr_state = match state {
            libvsched2::TaskState::Running => TaskState::Running,
            libvsched2::TaskState::Ready => TaskState::Runable,
            libvsched2::TaskState::Blocked => TaskState::Blocked,
            libvsched2::TaskState::Exited => TaskState::Exited,
        };
        TaskInner::set_state(self, curr_state);
        prev_state
    }

    #[doc = r" 任务优先级"]
    fn priority(&self) -> isize {
        0
    }

    #[doc = r" 判断任务为线程或协程，依据是保存的上下文类型"]
    #[doc = r""]
    #[doc = r" 根据最新保存的上下文类型不同，线程和协程可以互相转化"]
    fn is_coroutine(&self) -> bool {
        #[cfg(feature = "thread")]
        {
            self.have_stack_ctx()
        }
        #[cfg(not(feature = "thread"))]
        {
            true
        }
    }

    #[doc = r" 获取任务所处的进程id，也就是任务所处地址空间的所属进程的id，"]
    #[doc = r" 因此某些内核态任务也可能属于某个进程。"]
    #[doc = r""]
    #[doc = r" 如果之前未对该任务调用过`set_pid`，则返回0。否则，返回上一次`set_pid`传入的值。"]
    #[doc = r""]
    #[doc = r" 目前，因为该字段仅用于获取内核任务所处的地址空间，"]
    #[doc = r" 且因为任务的创建由os负责，无法在每个任务创建时均设置其pid，"]
    #[doc = r" 所以仅对于由进程创建的内核态任务（如同步/异步trap处理任务），"]
    #[doc = r" 我们会使用`set_pid`设置其pid，"]
    #[doc = r" 也只有对这些任务调用`pid`才能获得有效的值。"]
    #[doc = r""]
    #[doc = r" 此处的进程id即为全局进程表`PROCESS_INFO_TABLE`的索引"]
    fn pid(&self) -> usize {
        self.get_process_id() as usize
    }

    #[doc = r" 设置任务的pid，也就是任务所处地址空间的所属进程的id，"]
    #[doc = r" 此处的进程id即为全局进程表`PROCESS_INFO_TABLE`的索引。"]
    #[doc = r""]
    #[doc = r" 目前仅对于由进程创建的内核态任务（如同步/异步trap处理任务）调用，"]
    #[doc = r" 因此只有对这些任务调用`pid`才能获得有效的值。"]
    fn set_pid(&self, pid: usize) {
        self.set_process_id(pid as u64);
    }

    // #[doc = r" 保存线程上下文"]
    // fn save_thread_context(&self) {
    //     todo!()
    // }

    // #[doc = r" 保存trap上下文"]
    // fn save_trap_context(&self) {
    //     todo!()
    // }

    #[doc = r" 恢复寄存器上下文（可能为线程上下文或trap上下文）"]
    fn restore_context(&self) {
        // restore_from_stack_ctx接收的参数为&Arc类型，不涉及引用计数更改。此处用Arc包装只是为了适配已有的接口。
        let self_ref = unsafe { ManuallyDrop::new(Arc::from_raw(self as *const Task)) };
        restore_from_stack_ctx(&self_ref);
    }

    #[doc = r" 恢复协程上下文，函数返回时自动保存了协程上下文"]
    fn poll(&self) -> Poll<isize> {
        let waker = curr.waker();
        let cx = &mut Context::from_waker(&waker);
        self.get_fut().as_mut().poll(cx)
    }

    #[doc = r" 获取线程上下文保存的栈底指针"]
    fn thread_stack_base(&self) -> usize {
        assert!(!self.is_coroutine());
        self.stack_top()
    }

    #[doc = r" 设置协程运行返回值"]
    fn set_return_value(&self, value: isize) {
        self.set_exit_code(value)
    }
}
