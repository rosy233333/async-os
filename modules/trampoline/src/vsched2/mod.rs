use core::{
    alloc::Layout,
    future::Future,
    mem::ManuallyDrop,
    task::{Context, Poll, Waker},
};

use alloc::{boxed::Box, sync::Arc};
use async_mem::MemorySet;
use axhal::{
    arch::send_ipi,
    mem::{phys_to_virt, VirtAddr},
};
use executor::{current_task, current_task_may_uninit, KERNEL_EXECUTOR};
use kernel_guard::{BaseGuard, NoPreemptIrqSave};
use sync::Mutex;
use taskctx::{TaskInner, TaskStack, TaskState, TrapFrame};
use vdso::VVAR;

// mod stack;
pub mod task;

#[repr(transparent)]
struct Task(taskctx::Task);

impl libvsched2::Task for Task {
    #[doc = r" 任务状态"]
    fn state(&self) -> libvsched2::TaskState {
        log::debug!("Calling Task::state.");
        let state = TaskInner::state(&self.0);
        let res = match state {
            TaskState::Running => libvsched2::TaskState::Running,
            TaskState::Runable => libvsched2::TaskState::Ready,
            TaskState::Waked => libvsched2::TaskState::Ready,
            TaskState::Blocked => libvsched2::TaskState::Blocked,
            TaskState::Blocking => libvsched2::TaskState::Blocking,
            TaskState::Exited => libvsched2::TaskState::Exited,
        };
        log::debug!("Returned from Task::state.");
        res
    }

    #[doc = r" 设置任务状态"]
    fn set_state(&self, state: libvsched2::TaskState) -> libvsched2::TaskState {
        log::debug!("Calling Task::set_state.");
        let mut guard = self.0.state_lock_manual();
        let prev_state = **guard;
        let curr_state = match state {
            libvsched2::TaskState::Running => TaskState::Running,
            libvsched2::TaskState::Ready => TaskState::Runable,
            libvsched2::TaskState::Blocked => TaskState::Blocked,
            libvsched2::TaskState::Exited => TaskState::Exited,
            libvsched2::TaskState::Blocking => TaskState::Blocking,
        };
        **guard = curr_state;
        drop(ManuallyDrop::into_inner(guard));
        log::debug!("Returned from Task::set_state.");
        match prev_state {
            TaskState::Running => libvsched2::TaskState::Running,
            TaskState::Runable => libvsched2::TaskState::Ready,
            TaskState::Waked => libvsched2::TaskState::Ready,
            TaskState::Blocked => libvsched2::TaskState::Blocked,
            TaskState::Blocking => libvsched2::TaskState::Blocking,
            TaskState::Exited => libvsched2::TaskState::Exited,
        }
    }

    #[doc = r" 根据任务当前的状态，修改任务状态为参数中的对应值。返回任务的旧状态。"]
    #[doc = r""]
    #[doc = r" 该接口对任务状态的所有比较和修改需要实现为一个原子操作，从而防止多核下任务阻塞和唤醒相关的同步问题。"]
    fn match_set_state(
        &self,
        state_from_ready: libvsched2::TaskState,
        state_from_running: libvsched2::TaskState,
        state_from_blocked: libvsched2::TaskState,
        state_from_exited: libvsched2::TaskState,
        state_from_blocking: libvsched2::TaskState,
    ) -> libvsched2::TaskState {
        log::debug!("Calling Task::match_set_state.");
        let mut guard = self.0.state_lock_manual();
        let prev_state = **guard;
        let state = match prev_state {
            TaskState::Running => state_from_running,
            TaskState::Runable => state_from_ready,
            TaskState::Blocked => state_from_blocked,
            TaskState::Exited => state_from_exited,
            TaskState::Blocking => state_from_blocking,
            TaskState::Waked => state_from_ready,
        };
        let curr_state = match state {
            libvsched2::TaskState::Running => TaskState::Running,
            libvsched2::TaskState::Ready => TaskState::Runable,
            libvsched2::TaskState::Blocked => TaskState::Blocked,
            libvsched2::TaskState::Exited => TaskState::Exited,
            libvsched2::TaskState::Blocking => TaskState::Blocking,
        };
        **guard = curr_state;
        drop(ManuallyDrop::into_inner(guard));
        log::debug!("Returned from Task::set_state.");
        match prev_state {
            TaskState::Running => libvsched2::TaskState::Running,
            TaskState::Runable => libvsched2::TaskState::Ready,
            TaskState::Waked => libvsched2::TaskState::Ready,
            TaskState::Blocked => libvsched2::TaskState::Blocked,
            TaskState::Blocking => libvsched2::TaskState::Blocking,
            TaskState::Exited => libvsched2::TaskState::Exited,
        }
    }
    #[doc = r" 任务优先级"]
    fn priority(&self) -> isize {
        log::debug!("Calling and returned from Task::priority.");
        0
    }

    #[doc = r" 判断任务为线程或协程，依据是保存的上下文类型"]
    #[doc = r""]
    #[doc = r" 根据最新保存的上下文类型不同，线程和协程可以互相转化"]
    fn is_coroutine(&self) -> bool {
        log::debug!("Calling Task::is_coroutine.");
        // #[cfg(feature = "thread-api")]
        let res = { !self.0.have_stack_ctx() };
        // #[cfg(not(feature = "thread-api"))]
        // let res = { true };
        log::debug!("Returned from Task::is_coroutine.");
        res
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
        log::debug!("Calling Task::pid.");
        let res = self.0.get_process_id() as usize;
        log::debug!("Returned from Task::pid.");
        res
    }

    #[doc = r" 设置任务的pid，也就是任务所处地址空间的所属进程的id，"]
    #[doc = r" 此处的进程id即为全局进程表`PROCESS_INFO_TABLE`的索引。"]
    #[doc = r""]
    #[doc = r" 目前仅对于由进程创建的内核态任务（如同步/异步trap处理任务）调用，"]
    #[doc = r" 因此只有对这些任务调用`pid`才能获得有效的值。"]
    fn set_pid(&self, pid: usize) {
        log::debug!("Calling Task::set_pid.");
        self.0.set_process_id(pid as u64);
        log::debug!("Returned from Task::set_pid.");
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
        // #[cfg(any(feature = "thread-api", feature = "preempt"))]
        // {
        //     log::debug!("Calling Task::restore_context.");
        //     use crate::restore_from_stack_ctx;
        //     // restore_from_stack_ctx接收的参数为&Arc类型，不涉及引用计数更改。此处用Arc包装只是为了适配已有的接口。
        //     let self_ref = unsafe {
        //         ManuallyDrop::new(Arc::from_raw(self as *const Task as *const taskctx::Task))
        //     };
        //     log::debug!("Task::restore_context: before restore");
        //     restore_from_stack_ctx(&self_ref);
        // }
        // #[cfg(not(any(feature = "thread-api", feature = "preempt")))]
        // {
        //     todo!()
        // }
        log::debug!("Calling Task::restore_context.");
        use crate::restore_from_stack_ctx;
        // restore_from_stack_ctx接收的参数为&Arc类型，不涉及引用计数更改。此处用Arc包装只是为了适配已有的接口。
        let self_ref = unsafe {
            ManuallyDrop::new(Arc::from_raw(self as *const Task as *const taskctx::Task))
        };
        log::debug!("Task::restore_context: before restore");
        restore_from_stack_ctx(&self_ref);
        panic!("`restore_context`: unreachable!");
    }

    #[doc = r" 恢复协程上下文，函数返回时自动保存了协程上下文"]
    fn poll(&self) -> Poll<isize> {
        log::debug!("Calling Task::poll.");
        let waker = taskctx::waker_from_task(&self.0 as *const _);
        let cx = &mut Context::from_waker(&waker);
        // let sip = riscv::register::sip::read();
        // log::info!(
        //     "poll: \nsip: timer: {}, software: {}, external: {}",
        //     sip.stimer(),
        //     sip.ssoft(),
        //     sip.sext()
        // );

        // // 目前还未考虑如何进行开关中断的设置，以兼容“处理了开关中断的Future”和“未处理开关中断的Future”。
        // let state = NoPreemptIrqSave::acquire();
        let res = self.0.get_fut().as_mut().poll(cx);
        // NoPreemptIrqSave::release(state);
        log::debug!("Returned from Task::poll.");
        res
    }

    #[doc = r" 设置协程运行返回值"]
    fn set_return_value(&self, value: isize) {
        log::debug!("Calling Task::set_return_value.");
        self.0.set_exit_code(value);
        log::debug!("Returned from Task::set_return_value.");
    }

    #[doc = r" 判断任务是否为内核态任务"]
    fn is_kernel(&self) -> bool {
        log::debug!("Calling and returned from Task::is_kernel.");
        true
    }

    #[doc = r" 保存线程上下文，然后进入`api::raw_thread_entry`。"]
    #[doc = r" 下次从上下文中恢复后，从该函数中返回。"]
    #[doc = r""]
    #[doc = r" 不需修改任务状态。"]
    #[doc = r""]
    #[doc = r" 调用此函数时，`self`一定是当前任务。"]
    fn resched(&self) {
        log::debug!("Calling Task::resched.");
        // #[cfg(any(feature = "thread-api", feature = "preempt"))]
        task::resched();
        // #[cfg(not(any(feature = "thread-api", feature = "preempt")))]
        // panic!("Do not support thread reschedule!");
    }

    #[doc = r" 获取线程上下文保存的`Stack`指针"]
    fn thread_stack(&self) -> *mut () {
        // #[cfg(any(feature = "thread-api", feature = "preempt"))]
        // {
        //     log::debug!("Calling Task::thread_stack.");
        //     let res = self.0.stack() as *const _ as *const () as *mut ();
        //     log::debug!("Returned from Task::thread_stack.");
        //     res
        // }
        // #[cfg(not(any(feature = "thread-api", feature = "preempt")))]
        // {
        //     panic!("Do not support thread stack!");
        // }
        log::debug!("Calling Task::thread_stack.");
        let res = Box::into_raw(self.0.get_stack()) as *mut _;
        log::debug!("Returned from Task::thread_stack.");
        res
    }

    #[doc = r" 释放一个已经退出的任务"]
    fn dealloc(&self) {
        log::debug!("Calling Task::dealloc.");

        // 释放一个引用计数
        let to_drop = unsafe { Arc::from_raw(self as *const Task as *const taskctx::Task) };
        if to_drop.is_init() {
            axhal::misc::terminate();
        }
        to_drop.notify_waker_for_exit();
        drop(to_drop);
        log::debug!("Returned from Task::dealloc.");
    }
}

/// 栈的分配和回收。
///
/// 只会在栈所在的地址空间中调用。
///
/// 通过`Box`实现传指针，传入时先放入`Box`再`into_raw`转换为指针，传出时先`from_raw`转换为`Box`再根据需要使用`*`从`Box`中取出
#[repr(transparent)]
struct Stack(taskctx::TaskStack);

impl libvsched2::Stack for Stack {
    /// 分配栈
    fn alloc() -> *mut () {
        log::debug!("Calling Stack::alloc.");
        let stack = Box::new(Stack(TaskStack::alloc(axconfig::TASK_STACK_SIZE)));
        let res = Box::into_raw(stack) as *mut ();
        log::debug!("Returned from Stack::alloc: {:#x}", res as usize);
        res
    }
    /// 回收栈
    fn dealloc(&mut self) {
        log::debug!("Calling Stack::dealloc: {:#x}.", self as *mut _ as usize);
        let to_drop = unsafe { Box::from_raw(self as *mut Stack) };
        drop(to_drop);
        log::debug!("Returned from Stack::dealloc.");
    }

    #[doc = r" 栈底指针"]
    fn base(&self) -> *mut () {
        log::debug!("Calling Stack::base.");
        let res = self.0.top().as_mut_ptr() as *mut ();
        log::debug!("Returned from Stack::base.");
        res
    }
}

/// 特权级切换和地址空间切换
struct ContextImpl;

impl libvsched2::Context for ContextImpl {
    /// 在调度中陷入内核态，在空栈中进入`ktrap_entry`函数并在后续进入`utok_schedule`函数
    fn into_kernel() -> ! {
        panic!("Should not call `into_kernel` in kernel!");
    }
    /// 在调度中进入用户态，在空栈中进入`run_task`函数
    ///
    /// 参数为进入用户态时应该使用的用户栈的栈顶地址，即sp寄存器的值
    ///
    /// 在内核态调度到用户协程后使用
    fn into_user(ustack: usize) {
        log::debug!("Calling Context::into_user.");
        let vspace = libvsched2::current_vspace() as *mut ();
        assert!(!vspace.is_null());
        let memory_set = unsafe { &mut *(vspace as *mut Mutex<MemorySet>) };
        let vvar_base = memory_set.lock().vvar_base.as_usize();
        assert!(vvar_base != 0);
        let kernel_vvar_base = &VVAR as *const _ as usize;
        // `raw_run_task`在用户空间中的映射地址
        let user_run_task_ptr: *const () =
            (unsafe { libvsched2::VDSO_VTABLE.raw_run_task.unwrap() } as usize + vvar_base
                - kernel_vvar_base) as *const ();

        let tf = TrapFrame::init_user_context(user_run_task_ptr as usize, ustack);
        log::debug!("Context::into_user: Before user return.");
        unsafe {
            tf.user_return();
        }
        panic!("`into_user`: unreachable!");
    }
    /// 在调度中进入用户态寄存器上下文
    ///
    /// 参数中的指针指向外部定义的Task类型
    ///
    /// 在内核态调度到用户线程后使用
    fn into_user_context(task: *const ()) {
        log::debug!("Calling Context::into_user_context.");
        let task_ref = unsafe { &*(task as *const taskctx::Task) };
        let tf = task_ref.utrap_frame().unwrap();
        log::debug!("Context::into_user_context: Before user return.");
        unsafe {
            tf.user_return();
        }
        panic!("`into_user_context`: unreachable!");
    }
}

/// 同步trap处理
struct TrapInfo(TrapFrame);

impl libvsched2::TrapInfo for TrapInfo {
    #[doc = r" 从被trap的任务中获取trap信息"]
    #[doc = r""]
    #[doc = r" 传入的任务一定是被trap的任务，因此具有trap上下文类型的寄存器上下文。"]
    fn from_task(task: *const ()) -> *const Self {
        log::debug!("Calling TrapInfo::from_task with task {:#x}", task as usize);
        let task = unsafe { &*(task as *const taskctx::Task) };
        let frame = task.trap_frame().unwrap();
        let res = Box::into_raw(Box::new(TrapInfo(frame.clone())));
        log::debug!("Returned from TrapInfo::from_task.");
        res
    }

    #[doc = r" 处理trap。参数为被trap的任务。"]
    #[doc = r" 当被trap的任务与trap处理无关时（例如外部中断），参数为None。"]
    fn handle(&self, task: Option<*const ()>) {
        log::debug!("Calling TrapInfo::handle.");
        block_on(vsched2_trap_handle(&self.0, task));
        log::debug!("Returned from TrapInfo::handle.");
    }

    #[doc = r" 释放trap信息。"]
    #[doc = r""]
    #[doc = r" 在调度模块中，每个由`from_task`创建的`TrapInfo`实例都必须调用一次`dealloc`进行释放。"]
    fn dealloc(&self) {
        log::debug!("Calling TrapInfo::dealloc.");
        let to_drop = unsafe {
            Box::from_raw(self as *const TrapInfo as *mut TrapInfo);
        };
        drop(to_drop);
        log::debug!("Returned from TrapInfo::dealloc.");
    }

    #[doc = r" 创建一个新的trap处理任务，并返回TCB地址"]
    #[doc = r" （指向impl Task的指针）"]
    #[doc = r""]
    #[doc = r" trap处理任务使用`trap_handler`作为执行的函数，且将该函数的参数传入`trap_handler`中。"]
    fn new_handler(queue: *const ()) -> *const () {
        log::debug!("Calling TrapInfo::new_handler.");
        let task_ref = block_on(KERNEL_EXECUTOR.new_ktask(
            "trap_handler".into(),
            Box::pin(async move {
                libvsched2::api::trap_handler(queue);
                0
            }),
        ));
        let res = Arc::into_raw(task_ref) as *const ();
        log::debug!("Returned from TrapInfo::new_handler.");
        res
    }
}

// TODO: 目前只支持内核trap_frame，还不支持用户trap_frame。
async fn vsched2_trap_handle(tf: &TrapFrame, task: Option<*const ()>) {
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        use riscv::register::scause::{Exception, Trap};
        use syscall::trap::{handle_page_fault, MappingFlags};

        axhal::arch::disable_irqs();
        info!("into vsched2 trap handle");
        let trap = tf.get_scause_type();
        let stval = tf.stval;
        match trap {
            Trap::Interrupt(_interrupt) => {
                info!("into interrupt handle");
                crate::handle_user_irq(tf.get_scause_code()).await;
            }
            Trap::Exception(Exception::UserEnvCall) => {
                info!("into syscall handle");
                // axhal::arch::enable_irqs();
                // tf.sepc += 4;
                let task = unsafe { &*(task.unwrap() as *const Task) };
                if libvsched2::Task::is_kernel(task) {
                    panic!(
                        "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                        tf.get_scause_type(),
                        tf.sepc,
                        tf
                    );
                } else {
                    let task_tf = task.0.utrap_frame().unwrap();
                    task_tf.sepc += 4;
                    // 简单的方式是根据参数的值进行不同的处理，根据参数进行不同的处理
                    let result = syscall::trap::handle_syscall(
                        tf.regs.a7,
                        [
                            tf.regs.a0, tf.regs.a1, tf.regs.a2, tf.regs.a3, tf.regs.a4, tf.regs.a5,
                        ],
                    )
                    .await;
                    // 判断任务是否退出
                    // if curr.is_exited() {
                    // // 任务结束，需要切换至其他任务，关中断
                    //      axhal::arch::disable_irqs();
                    //      return curr.get_exit_code() as isize;
                    // }
                    if -result == syscall::SyscallError::ERESTART as isize {
                        // Restart the syscall
                        task_tf.rewind_pc();
                    } else {
                        task_tf.regs.a0 = result as usize;
                    }
                    // axhal::arch::disable_irqs();
                }
            }
            Trap::Exception(Exception::InstructionPageFault) => {
                // warn!("tf {:#X?}", tf);
                info!("into InstructionPageFault handle");
                let task = unsafe { &*(task.unwrap() as *const Task) };
                if libvsched2::Task::is_kernel(task) {
                    panic!(
                        "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                        tf.get_scause_type(),
                        tf.sepc,
                        tf
                    );
                } else {
                    handle_page_fault(stval.into(), MappingFlags::USER | MappingFlags::EXECUTE)
                        .await;
                }
            }

            Trap::Exception(Exception::LoadPageFault) => {
                // warn!("tf {:#X?}", tf);
                info!("into LoadPageFault handle");
                let task = unsafe { &*(task.unwrap() as *const Task) };
                if libvsched2::Task::is_kernel(task) {
                    panic!(
                        "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                        tf.get_scause_type(),
                        tf.sepc,
                        tf
                    );
                } else {
                    handle_page_fault(stval.into(), MappingFlags::USER | MappingFlags::READ).await;
                }
            }

            Trap::Exception(Exception::StorePageFault) => {
                // warn!("tf {:#X?}", tf);
                info!("into StorePageFault handle");
                let task = unsafe { &*(task.unwrap() as *const Task) };
                if libvsched2::Task::is_kernel(task) {
                    panic!(
                        "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                        tf.get_scause_type(),
                        tf.sepc,
                        tf
                    );
                } else {
                    handle_page_fault(stval.into(), MappingFlags::USER | MappingFlags::WRITE).await;
                }
            }

            _ => {
                panic!(
                    "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                    tf.get_scause_type(),
                    tf.sepc,
                    tf
                );
            }
        }
        // TODO: 在被中断的是用户进程时，处理信号。
        // info!("before signal handle");
        // syscall::trap::handle_signals().await;

        axhal::arch::disable_irqs();
    }
}

/// 多核接口
///
/// 除了此处以外，还包括编译时通过环境变量修改的核心数`CPU_NUM`。
struct SMPImpl;

impl libvsched2::SMP for SMPImpl {
    /// 获取当前cpuid
    fn cpu_id() -> usize {
        log::debug!("Calling SMP::cpu_id.");
        let res = axhal::cpu::this_cpu_id();
        log::debug!("Returned from SMP::cpu_id.");
        res
    }

    #[doc = r" 发送核间中断，用于唤醒正在睡眠的核心"]
    fn send_ipi(target_cpu: usize) {
        send_ipi(target_cpu);
    }
}

/// 地址空间相关接口
///
/// `*mut ()`为指向`Executor`中`Mutex<MemorySet>`的指针，且由`Arc::into_raw`产生。
#[repr(transparent)]
struct VSpaceImpl(Mutex<MemorySet>);

impl libvsched2::VSpace for VSpaceImpl {
    /// 切换地址空间
    ///
    /// 地址空间使用`*mut ()`表示，即为`ProcessInfo`中的`vspace`中的内容。
    fn into_vspace(&self) {
        log::debug!("Calling VSPace::into_vspace.");
        let page_table_token = self.0.lock().page_table_token();
        if page_table_token != 0 {
            unsafe {
                axhal::arch::write_page_table_root0(page_table_token.into());
                #[cfg(target_arch = "riscv64")]
                riscv::register::sstatus::set_sum();
            };
        }
        log::debug!("Returned from VSPace::into_vspace.");
    }

    #[doc = r" 释放表示地址空间的相应数据结构"]
    #[doc = r""]
    #[doc = r" 该数据结构在`process_init`中传入vDSO并存储于`PROCESS_INFO_TABLE`中，"]
    #[doc = r" 并在`process_drop`函数中释放。"]
    #[doc = r""]
    #[doc = r" 不一定要释放页表，例如可能只是降低了引用计数"]
    fn dealloc(&self) {
        log::debug!("Calling VSPace::dealloc.");
        let to_drop =
            unsafe { Arc::from_raw(self as *const _ as *const () as *const Mutex<MemorySet>) };
        drop(to_drop);
        log::debug!("Returned from VSPace::dealloc.");
    }
}

struct UserDataImpl;

impl libvsched2::UserData for UserDataImpl {
    /// 从内核中访问用户态vDSO私有数据
    ///
    /// 因为即使在一个地址空间中，用户态和内核态的vDSO私有数据也是分开的，因此需要借助这个函数进行地址运算，获得用户态对应数据的引用。
    ///
    /// - `pos` ：为私有数据对象的地址。
    /// - `len` ：为对象的字节长度。
    /// - `vspace` ：为要访问的地址空间，如果为None则访问当前地址空间。
    /// - 返回值：为用户态vDSO私有数据区中对应对象的地址，调用方可以将其转换为对应类型的引用。
    ///
    /// # Safety
    ///
    /// - 外界实现的地址翻译必须保证返回的地址在用户态vDSO私有数据区内，且`[addr, addr + size_of::<T>())`完整可访问。
    /// - 因为访问的是用户态子空间的数据，因此不能在切换地址空间前后访问该函数返回的同一份引用。
    fn get_user_data(pos: usize, len: usize, vspace: Option<*mut ()>) -> *mut () {
        log::debug!("Calling UserData::get_user_data.");
        let vspace = vspace.unwrap_or(libvsched2::current_vspace() as *mut ());
        assert!(!vspace.is_null());
        let memory_set = unsafe { &mut *(vspace as *mut Mutex<MemorySet>) };
        let vvar_base = memory_set.lock().vvar_base.as_usize();
        assert!(vvar_base != 0);
        let kernel_vvar_base = unsafe { &VVAR as *const _ as usize };
        let vaddr = VirtAddr::from(pos + vvar_base - kernel_vvar_base);
        let paddr = memory_set.lock().query(vaddr).unwrap().0;
        let res = phys_to_virt(paddr).as_mut_ptr() as *mut ();
        log::debug!("Returned from UserData::get_user_data.");
        res
    }
}

fn block_on<F: Future>(fut: F) -> F::Output {
    let mut pinned_fut = Box::pin(fut);
    loop {
        if let Poll::Ready(output) = pinned_fut
            .as_mut()
            .poll(&mut Context::from_waker(&Waker::noop()))
        {
            return output;
        }
    }
}

pub(crate) fn init_vsched2() {
    info!("into init_vsched2()");
    libvsched2::init_vtable_Context::<ContextImpl>();
    libvsched2::init_vtable_SMP::<SMPImpl>();
    libvsched2::init_vtable_Stack::<Stack>();
    libvsched2::init_vtable_Task::<Task>();
    libvsched2::init_vtable_TrapInfo::<TrapInfo>();
    libvsched2::init_vtable_UserData::<UserDataImpl>();
    libvsched2::init_vtable_VSpace::<VSpaceImpl>();

    let mut init_stack = Box::new(Stack(TaskStack::new_init()));
    let init_task = KERNEL_EXECUTOR.new_ktask_init("boot".into(), Box::pin(async { 0 }));
    libvsched2::kernel_init_main(
        Box::into_raw(init_stack) as _,
        Arc::into_raw(init_task) as _,
    );
}

#[cfg(feature = "smp")]
pub(crate) fn init_vsched2_secondary() {
    let mut init_stack = Box::new(Stack(TaskStack::new_init()));
    let init_task = KERNEL_EXECUTOR.new_ktask_init("boot".into(), Box::pin(async { 0 }));
    libvsched2::kernel_init_secondary(
        Box::into_raw(init_stack) as _,
        Arc::into_raw(init_task) as _,
    );
}
