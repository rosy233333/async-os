
use core::{
    future::Future, 
    pin::Pin, 
    sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering}
};
use alloc::{
    boxed::Box, collections::btree_map::BTreeMap, format, string::{String, ToString}, sync::Arc, vec::Vec, vec
};
use axerrno::{AxError, AxResult};
use async_fs::api::{FileIO, OpenFlags};
use async_mem::MemorySet;
use axhal::{mem::VirtAddr, time::current_time_nanos};
use axsignal::signal_no::SignalNo;
use lazy_init::LazyInit;
use taskctx::{BaseScheduler, Task, TaskInner, TaskRef, TrapFrame};
use spinlock::{SpinNoIrq, SpinNoIrqOnly};
use sync::Mutex;
use taskctx::{Scheduler, TaskId};
use crate::{
    current_task, fd_manager::{FdManager, FdTable}, flags::CloneFlags, load_app, stdio::{Stderr, Stdin, Stdout}, ExecutorRef, SignalModule
};
use task_api::yield_now;

const FD_LIMIT_ORIGIN: usize = 1025;
pub const KERNEL_EXECUTOR_ID: u64 = 1;
pub static TID2TASK: Mutex<BTreeMap<u64, TaskRef>> = Mutex::new(BTreeMap::new());
pub static PID2PC: Mutex<BTreeMap<u64, Arc<Executor>>> = Mutex::new(BTreeMap::new());

pub static UTRAP_HANDLER: LazyInit<fn() -> Pin<Box<dyn Future<Output = i32> + 'static>>> = LazyInit::new();

pub static KERNEL_EXECUTOR: LazyInit<Arc<Executor>> = LazyInit::new();
pub(crate) static EXECUTORS: SpinNoIrqOnly<BTreeMap<u64, ExecutorRef>> = SpinNoIrqOnly::new(BTreeMap::new());

extern "C" {
    fn start_signal_trampoline();
}

pub struct Executor {
    pub pid: TaskId,
    pub parent: AtomicU64,
    /// 子进程
    pub children: Mutex<Vec<Arc<Executor>>>,
    scheduler: Arc<SpinNoIrq<Scheduler>>,
    /// 文件描述符管理器
    pub fd_manager: FdManager,
    /// 进程状态
    pub is_zombie: AtomicBool,
    /// 退出状态码
    pub exit_code: AtomicI32,

    /// 地址空间
    pub memory_set: Arc<Mutex<MemorySet>>,
    /// 用户堆基址，任何时候堆顶都不能比这个值小，理论上讲是一个常量
    pub heap_bottom: AtomicU64,
    /// 当前用户堆的堆顶，不能小于基址，不能大于基址加堆的最大大小
    pub heap_top: AtomicU64,
    /// 是否被vfork阻塞
    pub blocked_by_vfork: Mutex<bool>,
    /// 该进程可执行文件所在的路径
    pub file_path: Mutex<String>,
    /// 信号处理模块
    /// 第一维代表TaskID，第二维代表对应的信号处理模块
    pub signal_modules: Mutex<BTreeMap<u64, SignalModule>>,
    /// 栈大小
    pub stack_size: AtomicU64,
    pub main_task: Mutex<Option<TaskRef>>,
}

unsafe impl Sync for Executor {}
unsafe impl Send for Executor {}

impl Executor {
    
    /// 创建一个新的 Executor（进程）
    pub fn new(
        pid: TaskId,
        parent: u64,
        memory_set: Arc<Mutex<MemorySet>>,
        heap_bottom: u64,
        fd_table: FdTable,
        cwd: Arc<Mutex<String>>,
        mask: Arc<AtomicI32>,
    ) -> Self {
        let mut scheduler = Scheduler::new();
        scheduler.init();
        Self {
            pid,
            parent: AtomicU64::new(parent),
            children: Mutex::new(Vec::new()),
            scheduler: Arc::new(SpinNoIrq::new(scheduler)),
            fd_manager: FdManager::new(fd_table, cwd, mask, FD_LIMIT_ORIGIN),
            is_zombie: AtomicBool::new(false),
            exit_code: AtomicI32::new(0),
            memory_set,
            heap_bottom: AtomicU64::new(heap_bottom),
            heap_top: AtomicU64::new(heap_bottom),
            blocked_by_vfork: Mutex::new(false),
            file_path: Mutex::new(String::new()),
            signal_modules: Mutex::new(BTreeMap::new()),
            stack_size: AtomicU64::new(axconfig::TASK_STACK_SIZE as _),
            main_task: Mutex::new(None),
        }
    }

    /// 内核 Executor
    pub fn new_init() -> Self {
        let new_fd_table: FdTable = Arc::new(Mutex::new(vec![
            // 标准输入
            Some(Arc::new(Stdin {
                flags: Mutex::new(OpenFlags::empty()),
            })),
            // 标准输出
            Some(Arc::new(Stdout {
                flags: Mutex::new(OpenFlags::empty()),
            })),
            // 标准错误
            Some(Arc::new(Stderr {
                flags: Mutex::new(OpenFlags::empty()),
            })),
        ]));
        Executor::new(
            TaskId::new(), 
            KERNEL_EXECUTOR_ID, 
            Arc::new(Mutex::new(MemorySet::new_memory_set())),
            0,
            new_fd_table,
            Arc::new(Mutex::new(String::from("/"))),
            Arc::new(AtomicI32::new(0o022)),
        )
    }

    /// 获取调度器
    pub fn get_scheduler(&self) -> Arc<SpinNoIrq<Scheduler>> {
        self.scheduler.clone()
    }

    /// 获取 Executor（进程）id
    pub fn pid(&self) -> TaskId {
        self.pid
    }

    /// 获取父 Executor（进程）id
    pub fn get_parent(&self) -> u64 {
        self.parent.load(Ordering::Acquire)
    }

    /// 设置父 Executor（进程）id
    pub fn set_parent(&self, parent: u64) {
        self.parent.store(parent, Ordering::Release)
    }

    /// 获取 Executor（进程）退出码
    pub fn get_exit_code(&self) -> i32 {
        self.exit_code.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）退出码
    pub fn set_exit_code(&self, exit_code: i32) {
        self.exit_code.store(exit_code, Ordering::Release)
    }

    /// 判断 Executor（进程）是否处于僵尸状态
    pub fn get_zombie(&self) -> bool {
        self.is_zombie.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）是否处于僵尸状态
    pub fn set_zombie(&self, status: bool) {
        self.is_zombie.store(status, Ordering::Release)
    }

    /// 获取 Executor（进程）的堆顶
    pub fn get_heap_top(&self) -> u64 {
        self.heap_top.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）的堆顶
    pub fn set_heap_top(&self, top: u64) {
        self.heap_top.store(top, Ordering::Release)
    }

    /// 获取 Executor（进程）的堆底
    pub fn get_heap_bottom(&self) -> u64 {
        self.heap_bottom.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）的堆底
    pub fn set_heap_bottom(&self, bottom: u64) {
        self.heap_bottom.store(bottom, Ordering::Release)
    }

    /// set stack size
    pub fn set_stack_limit(&self, limit: u64) {
        self.stack_size.store(limit, Ordering::Release)
    }

    /// get stack size
    pub fn get_stack_limit(&self) -> u64 {
        self.stack_size.load(Ordering::Acquire)
    }

    /// 设置 Executor（进程）是否被 vfork 阻塞
    pub async fn set_vfork_block(&self, value: bool) {
        *self.blocked_by_vfork.lock().await = value;
    }

    /// 获取 Executor（进程）是否被 vfork 阻塞
    pub async fn get_vfork_block(&self) -> bool {
        *self.blocked_by_vfork.lock().await
    }

    /// 设置 Executor（进程）可执行文件路径
    pub async fn set_file_path(&self, path: String) {
        let mut file_path = self.file_path.lock().await;
        *file_path = path;
    }

    /// 获取 Executor（进程）可执行文件路径
    pub async fn get_file_path(&self) -> String {
        (*self.file_path.lock().await).clone()
    }

    /// 若进程运行完成，则获取其返回码
    /// 若正在运行（可能上锁或没有上锁），则返回None
    pub fn get_code_if_exit(&self) -> Option<i32> {
        if self.get_zombie() {
            return Some(self.get_exit_code());
        }
        None
    }

    #[inline]
    /// Pick one task from Executor
    pub fn pick_next_task(&self) -> Option<TaskRef> {
        self.scheduler
            .lock()
            .pick_next_task()
    }

    #[inline]
    /// Add curr task to Executor, it ususally add to back
    pub fn put_prev_task(&self, task: TaskRef, front: bool) {
        self.scheduler.lock().put_prev_task(task, front);
    }

    #[inline]
    /// Add task to Executor, now just put it to own Executor
    /// TODO: support task migrate on differ Executor
    pub fn add_task(task: TaskRef) {
        task.get_scheduler().lock().add_task(task);
    }

    #[inline]
    /// Executor Clean
    pub fn task_tick(&self, task: &TaskRef) -> bool {
        self.scheduler.lock().task_tick(task)
    }

    #[inline]
    /// Executor Clean
    pub fn set_priority(&self, task: &TaskRef, prio: isize) -> bool {
        self.scheduler.lock().set_priority(task, prio)
    }

    /// TODO：这里需要存在问题，当进程结束时，这个任务也会结束，需要将这个任务删除，目前 wait 等待进程结束时，退出的操作还不正确
    /// 在这里还需要回收 Executor 的操作
    #[inline]
    pub async fn run(self: Arc<Self>) -> i32 {
        use core::{future::poll_fn, task::Poll};
        let page_table_token = self.memory_set.lock().await.page_table_token();
        poll_fn(|cx| {
            if self.is_zombie.load(Ordering::Acquire) {
                return Poll::Ready(0);
            }
            crate::CurrentExecutor::clean_current();
            unsafe { crate::CurrentExecutor::init_current(self.clone()) };
            if page_table_token != 0 {
                unsafe {
                    axhal::arch::write_page_table_root0(page_table_token.into());
                    #[cfg(target_arch = "riscv64")]
                    riscv::register::sstatus::set_sum();
                    axhal::arch::flush_tlb(None);
                };
            }
            cx.waker().wake_by_ref();
            Poll::Pending
        }).await
    }

}

impl Drop for Executor {
    fn drop(&mut self) {
        log::debug!("drop executor");
    }
}

impl Executor {
    pub async fn set_main_task(&self, task: TaskRef) {
        *self.main_task.lock().await = Some(task);
    }

    pub async fn get_main_task(&self) -> Option<TaskRef> {
        self.main_task.lock().await.clone()
    }

    pub async fn exit_main_task(&self) -> Option<TaskRef> {
        self.main_task.lock().await.take()
    }
}

impl Executor {

    /// 获取当前进程的工作目录
    pub async fn get_cwd(&self) -> String {
        self.fd_manager.cwd.lock().await.clone().to_string()
    }

    /// Set the current working directory of the process
    pub async fn set_cwd(&self, cwd: String) {
        *self.fd_manager.cwd.lock().await = cwd.into();
    }

    /// alloc physical memory for lazy allocation manually
    pub async fn manual_alloc_for_lazy(&self, addr: VirtAddr) -> AxResult<()> {
        self.memory_set.lock().await.manual_alloc_for_lazy(addr).await
    }

    /// alloc range physical memory for lazy allocation manually
    pub async fn manual_alloc_range_for_lazy(&self, start: VirtAddr, end: VirtAddr) -> AxResult<()> {
        self.memory_set
            .lock().await
            .manual_alloc_range_for_lazy(start, end).await
    }

    /// alloc physical memory with the given type size for lazy allocation manually
    pub async fn manual_alloc_type_for_lazy<T: Sized>(&self, obj: *const T) -> AxResult<()> {
        self.memory_set
            .lock().await
            .manual_alloc_type_for_lazy(obj).await
    }

    /// 为进程分配一个文件描述符
    pub fn alloc_fd(&self, fd_table: &mut Vec<Option<Arc<dyn FileIO>>>) -> AxResult<usize> {
        for (i, fd) in fd_table.iter().enumerate() {
            if fd.is_none() {
                return Ok(i);
            }
        }
        if fd_table.len() >= self.fd_manager.get_limit() as usize {
            debug!("fd table is full");
            return Err(AxError::StorageFull);
        }
        fd_table.push(None);
        Ok(fd_table.len() - 1)
    }

    /// 查询当前任务是否存在未决信号
    pub async fn have_signals(&self) -> Option<usize> {
        let current_task = current_task();
        self.signal_modules
            .lock()
            .await
            .get(&current_task.id().as_u64())
            .unwrap()
            .signal_set
            .find_signal()
            .map_or_else(|| current_task.check_pending_signal(), Some)
    }

    /// Judge whether the signal request the interrupted syscall to restart
    ///
    /// # Return
    /// - None: There is no siganl need to be delivered
    /// - Some(true): The interrupted syscall should be restarted
    /// - Some(false): The interrupted syscall should not be restarted
    pub async fn have_restart_signals(&self) -> Option<bool> {
        let current_task = current_task();
        self.signal_modules
            .lock().await
            .get(&current_task.id().as_u64())
            .unwrap()
            .have_restart_signal()
            .await
    }
}

impl Executor {
    
    /// 根据给定参数创建一个新的 Executor
    /// 在这期间如果，如果任务从一个核切换到另一个核就会导致地址空间不正确，产生内核页错误
    pub async fn init_user(args: Vec<String>, envs: &Vec<String>) -> AxResult<TaskRef> {
        let mut path = args[0].clone();
        let mut memory_set = MemorySet::new_memory_set();
        {
            use axhal::mem::virt_to_phys;
            use axhal::paging::MappingFlags;
            // 生成信号跳板
            let signal_trampoline_vaddr: VirtAddr = (axconfig::SIGNAL_TRAMPOLINE).into();
            let signal_trampoline_paddr = virt_to_phys((start_signal_trampoline as usize).into());
            memory_set.map_page_without_alloc(
                signal_trampoline_vaddr,
                signal_trampoline_paddr,
                MappingFlags::READ
                    | MappingFlags::EXECUTE
                    | MappingFlags::USER
                    | MappingFlags::WRITE,
            )?;
        }
        let page_table_token = memory_set.page_table_token();
        if page_table_token != 0 {
            unsafe {
                axhal::arch::write_page_table_root0(page_table_token.into());
                #[cfg(target_arch = "riscv64")]
                riscv::register::sstatus::set_sum();
            };
        }
        log::debug!("write page table done");
        let (entry, user_stack_bottom, heap_bottom) =
            if let Ok(ans) = load_app(path.clone(), args, envs, &mut memory_set).await {
                ans
            } else {
                error!("Failed to load app {}", path);
                return Err(AxError::NotFound);
            };
        let new_fd_table: FdTable = Arc::new(Mutex::new(vec![
            // 标准输入
            Some(Arc::new(Stdin {
                flags: Mutex::new(OpenFlags::empty()),
            })),
            // 标准输出
            Some(Arc::new(Stdout {
                flags: Mutex::new(OpenFlags::empty()),
            })),
            // 标准错误
            Some(Arc::new(Stderr {
                flags: Mutex::new(OpenFlags::empty()),
            })),
        ]));
        let new_executor = Arc::new(Executor::new(
            TaskId::new(),
            KERNEL_EXECUTOR_ID,
            Arc::new(Mutex::new(memory_set)),
            heap_bottom.as_usize() as u64,
            new_fd_table,
            Arc::new(Mutex::new(String::from("/").into())),
            Arc::new(AtomicI32::new(0o022)),
        ));
        if !path.starts_with('/') {
            //如果path不是绝对路径, 则加上当前工作目录
            let cwd = new_executor.get_cwd().await;
            assert!(cwd.ends_with('/'));
            path = format!("{}{}", cwd, path);
        }
        new_executor.set_file_path(path.clone()).await;
        let scheduler = new_executor.get_scheduler();
        let fut = UTRAP_HANDLER();
        let pid = new_executor.pid().as_u64();
        let new_task = Arc::new(Task::new(
            TaskInner::new_user(
                path,
                pid,
                scheduler, 
                fut,
                Box::new(TrapFrame::init_user_context(
                    entry.into(), user_stack_bottom.into()
                ))
            )
        ));

        // let new_task = spawn_raw(|| run_user_task(entry), path);
        // let new_task = new_task(Box::pin(UserTask::new(entry, user_stack_bottom)), path);
        // Executor::add_task(new_task.clone());
        new_executor.get_scheduler().lock().add_task(new_task.clone());
        TID2TASK
            .lock().await
            .insert(new_task.id().as_u64(), Arc::clone(&new_task));
        new_task.set_leader(true);
        new_executor.set_main_task(new_task.clone()).await;

        new_executor
            .signal_modules
            .lock()
            .await
            .insert(new_task.id().as_u64(), SignalModule::init_signal(None));
        // new_process
        //     .robust_list
        //     .lock()
        //     .insert(new_task.id().as_u64(), FutexRobustList::default());
        PID2PC
            .lock().await
            .insert(new_executor.pid().as_u64(), Arc::clone(&new_executor));
        // // 将其作为内核进程的子进程
        // match PID2PC.lock().await.get(&KERNEL_PROCESS_ID) {
        //     Some(kernel_process) => {
        //         kernel_process.children.lock().await.push(new_process);
        //     }
        //     None => {
        //         return Err(Error::NotFound);
        //     }
        // }
        // spawn_raw(|| new_executor.run_self(), "executor".into());
        crate::spawn_raw(|| new_executor.run(), "executor".into());

        Ok(new_task)
    }

    /// 实现简易的clone系统调用
    /// 返回值为新产生的任务的id
    pub async fn clone_task(
        &self,
        flags: usize,
        stack: Option<usize>,
        ptid: usize,
        _tls: usize,
        ctid: usize,
        exit_signal: Option<SignalNo>,
    ) -> AxResult<u64> {
        let clone_flags = CloneFlags::from_bits((flags & !0x3f) as u32).unwrap();
        // 是否共享虚拟地址空间
        let new_memory_set = if clone_flags.contains(CloneFlags::CLONE_VM) {
            Arc::clone(&self.memory_set)
        } else {
            let memory_set = Arc::new(Mutex::new(MemorySet::clone_or_err(
                &mut *self.memory_set.lock().await,
            ).await?));

            {
                use axhal::mem::virt_to_phys;
                use axhal::paging::MappingFlags;
                // 生成信号跳板
                let signal_trampoline_vaddr: VirtAddr = (axconfig::SIGNAL_TRAMPOLINE).into();
                let signal_trampoline_paddr =
                    virt_to_phys((start_signal_trampoline as usize).into());
                memory_set.lock().await.map_page_without_alloc(
                    signal_trampoline_vaddr,
                    signal_trampoline_paddr,
                    MappingFlags::READ
                        | MappingFlags::EXECUTE
                        | MappingFlags::USER
                        | MappingFlags::WRITE,
                )?;
            }
            memory_set
        };

        // 在生成新的进程前，需要决定其所属进程是谁
        let process_id = if clone_flags.contains(CloneFlags::CLONE_THREAD) {
            // 当前clone生成的是线程，那么以self作为进程
            self.pid
        } else {
            // 新建一个进程，并且设计进程之间的父子关系
            TaskId::new()
        };
        // 决定父进程是谁
        let parent_id = if clone_flags.contains(CloneFlags::CLONE_PARENT) {
            // 创建兄弟关系，此时以self的父进程作为自己的父进程
            // 理论上不应该创建内核进程的兄弟进程，所以可以直接unwrap
            self.get_parent()
        } else {
            // 创建父子关系，此时以self作为父进程
            self.pid.as_u64()
        };
        // let new_task = new_task(
        //     || {},
        //     String::from(executor.tasks.lock()[0].name().split('/').last().unwrap()),
        //     executor.get_stack_limit() as usize,
        //     process_id,
        //     new_memory_set.lock().await.page_table_token(),
        // );
        let scheduler = self.get_scheduler();
        let fut = UTRAP_HANDLER();
        let utrap_frame = Box::new(*current_task().utrap_frame().unwrap());
        let new_task = Arc::new(Task::new(
            TaskInner::new_user(
                String::from(current_task().name().split('/').last().unwrap()),
                process_id.as_u64(),
                scheduler, 
                fut,
                utrap_frame,
            )
        ));

        // When clone a new task, the new task should have the same fs_base as the original task.
        //
        // It should be saved in trap frame, but to compatible with Unikernel, we save it in TaskContext.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            new_task.set_tls_force(async_axhal::arch::read_thread_pointer());
        }

        debug!("new task:{}", new_task.id_name());
        TID2TASK
            .lock()
            .await
            .insert(new_task.id().as_u64(), Arc::clone(&new_task));
        let new_handler = if clone_flags.contains(CloneFlags::CLONE_SIGHAND) {
            // let curr_id = current().id().as_u64();
            self.signal_modules
                .lock()
                .await
                .get_mut(&current_task().id().as_u64())
                .unwrap()
                .signal_handler
                .clone()
        } else {
            Arc::new(Mutex::new(
                self.signal_modules
                    .lock()
                    .await
                    .get_mut(&current_task().id().as_u64())
                    .unwrap()
                    .signal_handler
                    .lock()
                    .await
                    .clone(),
            ))
            // info!("curr_id: {:X}", (&curr_id as *const _ as usize));
        };
        // 检查是否在父任务中写入当前新任务的tid
        if clone_flags.contains(CloneFlags::CLONE_PARENT_SETTID)
            & self.manual_alloc_for_lazy(ptid.into()).await.is_ok()
        {
            unsafe {
                *(ptid as *mut i32) = new_task.id().as_u64() as i32;
            }
        }
        // 若包含CLONE_CHILD_SETTID或者CLONE_CHILD_CLEARTID
        // 则需要把线程号写入到子线程地址空间中tid对应的地址中
        if clone_flags.contains(CloneFlags::CLONE_CHILD_SETTID)
            || clone_flags.contains(CloneFlags::CLONE_CHILD_CLEARTID)
        {
            if clone_flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
                new_task.set_child_tid(ctid);
            }

            if clone_flags.contains(CloneFlags::CLONE_CHILD_CLEARTID) {
                new_task.set_clear_child_tid(ctid);
            }

            if clone_flags.contains(CloneFlags::CLONE_VM) {
                // 此时地址空间不会发生改变
                // 在当前地址空间下进行分配
                if self.manual_alloc_for_lazy(ctid.into()).await.is_ok() {
                    // 正常分配了地址
                    unsafe {
                        *(ctid as *mut i32) =
                            if clone_flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
                                new_task.id().as_u64() as i32
                            } else {
                                0
                            }
                    }
                } else {
                    return Err(AxError::BadAddress);
                }
            } else {
                // 否则需要在新的地址空间中进行分配
                let memory_set_wrapper = new_memory_set.lock().await;
                let mut vm = memory_set_wrapper;
                if vm.manual_alloc_for_lazy(ctid.into()).await.is_ok() {
                    // 此时token没有发生改变，所以不能直接解引用访问，需要手动查页表
                    if let Ok((phyaddr, _, _)) = vm.query(ctid.into()) {
                        let vaddr: usize = axhal::mem::phys_to_virt(phyaddr).into();
                        // 注意：任何地址都是从free memory分配来的，那么在页表中，free memory一直在页表中，他们的虚拟地址和物理地址一直有偏移的映射关系
                        unsafe {
                            *(vaddr as *mut i32) =
                                if clone_flags.contains(CloneFlags::CLONE_CHILD_SETTID) {
                                    new_task.id().as_u64() as i32
                                } else {
                                    0
                                }
                        }
                        drop(vm);
                    } else {
                        drop(vm);
                        return Err(AxError::BadAddress);
                    }
                } else {
                    drop(vm);
                    return Err(AxError::BadAddress);
                }
            }
        }
        // 返回的值
        // 若创建的是进程，则返回进程的id
        // 若创建的是线程，则返回线程的id
        let return_id: u64;
        // 决定是创建线程还是进程
        if clone_flags.contains(CloneFlags::CLONE_THREAD) {
            // // 若创建的是线程，那么不用新建进程
            // info!("task len: {}", inner.tasks.len());
            // info!("task address :{:X}", (&new_task as *const _ as usize));
            // info!(
            //     "task address: {:X}",
            //     (&Arc::clone(&new_task)) as *const _ as usize
            // );
            self.get_scheduler().lock().add_task(Arc::clone(&new_task));

            let mut signal_module = SignalModule::init_signal(Some(new_handler));
            // exit signal, default to be SIGCHLD
            if exit_signal.is_some() {
                signal_module.set_exit_signal(exit_signal.unwrap());
            }
            self.signal_modules
                .lock()
                .await
                .insert(new_task.id().as_u64(), signal_module);

            // self.robust_list
            //     .lock()
            //     .insert(new_task.id().as_u64(), FutexRobustList::default());
            return_id = new_task.id().as_u64();
        } else {
            let mut cwd_src = Arc::new(Mutex::new(String::from("/").into()));
            let mut mask_src = Arc::new(AtomicI32::new(0o022));
            if clone_flags.contains(CloneFlags::CLONE_FS) {
                cwd_src = Arc::clone(&self.fd_manager.cwd);
                mask_src = Arc::clone(&self.fd_manager.umask);
            }
            // 若创建的是进程，那么需要新建进程
            // 由于地址空间是复制的，所以堆底的地址也一定相同
            let fd_table = if clone_flags.contains(CloneFlags::CLONE_FILES) {
                Arc::clone(&self.fd_manager.fd_table)
            } else {
                Arc::new(Mutex::new(self.fd_manager.fd_table.lock().await.clone()))
            };
            let new_process = Arc::new(Executor::new(
                process_id, 
                parent_id, 
                new_memory_set, 
                self.get_heap_bottom(), 
                fd_table, 
                cwd_src, 
                mask_src
            ));
            // 复制当前工作文件夹
            new_process.set_cwd(self.get_cwd().await).await;
            // 记录该进程，防止被回收
            PID2PC.lock().await.insert(process_id.as_u64(), Arc::clone(&new_process));
            let scheduler = new_process.get_scheduler();
            new_task.set_scheduler(scheduler);
            new_task.set_leader(true);
            new_process.set_main_task(new_task.clone()).await;
            new_process.put_prev_task(new_task.clone(), false);
            // new_process.tasks.lock().push(Arc::clone(&new_task));
            // 若是新建了进程，那么需要把进程的父子关系进行记录
            let mut signal_module = SignalModule::init_signal(Some(new_handler));
            // exit signal, default to be SIGCHLD
            if exit_signal.is_some() {
                signal_module.set_exit_signal(exit_signal.unwrap());
            }
            new_process
                .signal_modules
                .lock()
                .await
                .insert(new_task.id().as_u64(), signal_module);

            // new_process
            //     .robust_list
            //     .lock()
            //     .insert(new_task.id().as_u64(), FutexRobustList::default());
            return_id = new_process.pid.as_u64();
            PID2PC
                .lock()
                .await
                .get_mut(&parent_id)
                .unwrap()
                .children
                .lock()
                .await
                .push(Arc::clone(&new_process));

            let scheduler = self.get_scheduler();
            let fut = Box::pin(new_process.run());
            let executor_task = Arc::new(Task::new(
                TaskInner::new(
                    "executor".into(),
                    process_id.as_u64(),
                    scheduler, 
                    fut,
                )
            ));
            self.get_scheduler().lock().add_task(executor_task);
        };

        // if !clone_flags.contains(CloneFlags::CLONE_THREAD) {
        //     new_task.set_leader(true);
        // }
        // 复制原有的trap上下文
        // let mut trap_frame = unsafe { *(current_task.get_first_trap_frame()) };
        // let mut trap_frame =
        //     read_trapframe_from_kstack(current_task.get_kernel_stack_top().unwrap());
        // drop(current_task);
        // 新开的进程/线程返回值为0
        let utrap_frame = new_task.utrap_frame().unwrap();
        utrap_frame.set_ret_code(0);
        utrap_frame.trap_status = taskctx::TrapStatus::Done;
        // if clone_flags.contains(CloneFlags::CLONE_SETTLS) {
        //     #[cfg(not(target_arch = "x86_64"))]
        //     trap_frame.set_tls(tls);
        //     #[cfg(target_arch = "x86_64")]
        //     unsafe {
        //         new_task.set_tls_force(tls);
        //     }
        // }

        // 设置用户栈
        // 若给定了用户栈，则使用给定的用户栈
        // 若没有给定用户栈，则使用当前用户栈
        // 没有给定用户栈的时候，只能是共享了地址空间，且原先调用clone的有用户栈，此时已经在之前的trap clone时复制了
        if let Some(stack) = stack {
            utrap_frame.set_user_sp(stack);
            // info!(
            //     "New user stack: sepc:{:X}, stack:{:X}",
            //     trap_frame.sepc, trap_frame.regs.sp
            // );
        }
        #[cfg(feature = "future")]
        {
            let stack_size = self.get_stack_limit() as usize;
            new_task.init_user_kstack(stack_size, core::mem::size_of::<TrapFrame>());
            new_task.set_ctx_type(axtask::ContextType::THREAD);
        }
        // write_trapframe_to_kstack(new_task.get_kernel_stack_top().unwrap(), &trap_frame);
        // Processor::first_add_task(new_task);
        // 判断是否为VFORK
        if clone_flags.contains(CloneFlags::CLONE_VFORK) {
            self.set_vfork_block(true).await;
            // VFORK: TODO: judge when schedule
            while self.get_vfork_block().await {
                yield_now().await;
            }
        }
        Ok(return_id)
    }

    /// 将当前进程替换为指定的用户程序
    /// args为传入的参数
    /// 任务的统计时间会被重置
    pub async fn exec(
        &self, 
        name: String, 
        args: Vec<String>, 
        envs: &Vec<String>
    ) -> AxResult<()> {
        // 首先要处理原先进程的资源
        // 处理分配的页帧
        // 之后加入额外的东西之后再处理其他的包括信号等因素
        // 不是直接删除原有地址空间，否则构建成本较高。

        if Arc::strong_count(&self.memory_set) == 1 {
            self.memory_set.lock().await.unmap_user_areas();
        } else {
            let memory_set = MemorySet::clone_or_err(
                &mut *self.memory_set.lock().await,
            ).await?;
            *self.memory_set.lock().await = memory_set;
            self.memory_set.lock().await.unmap_user_areas();
            let new_page_table = self.memory_set.lock().await.page_table_token();
            // let mut tasks = self.tasks.lock();
            // for task in tasks.iter_mut() {
            //     task.inner().set_page_table_token(new_page_table);
            // }
            // 切换到新的页表上
            unsafe {
                axhal::arch::write_page_table_root0(new_page_table.into());
            }
        }
        // 清空用户堆，重置堆顶
        axhal::arch::flush_tlb(None);

        // 关闭 `CLOEXEC` 的文件
        self.fd_manager.close_on_exec().await;
        let current_task = current_task();
        // 再考虑手动结束其他所有的task
        // 暂时不支持其他任务，因此直接注释掉下面的代码，并且目前的设计也不支持寻找其他的任务
        // let mut tasks = self.tasks.lock();
        // for _ in 0..tasks.len() {
        //     let task = tasks.pop().unwrap();
        //     if task.id() == current_task.id() {
        //         // FIXME: This will reset tls forcefully
        //         #[cfg(target_arch = "x86_64")]
        //         unsafe {
        //             task.set_tls_force(0);
        //             axhal::arch::write_thread_pointer(0);
        //         }
        //         tasks.push(task);
        //     } else {
        //         TID2TASK.lock().remove(&task.id().as_u64());
        //         panic!("currently not support exec when has another task ");
        //     }
        // }
        // 当前任务被设置为主线程
        current_task.set_leader(true);
        self.set_main_task(current_task.clone()).await;
        // 重置统计时间
        current_task.time_stat_reset(current_time_nanos() as usize);
        current_task.set_name(name.split('/').last().unwrap());
        // assert!(tasks.len() == 1);
        // drop(tasks);
        let args = if args.is_empty() {
            vec![name.clone()]
        } else {
            args
        };
        let (entry, user_stack_bottom, heap_bottom) = if let Ok(ans) =
            load_app(name.clone(), args, envs, &mut *self.memory_set.lock().await).await
        {
            ans
        } else {
            error!("Failed to load app {}", name);
            return Err(AxError::NotFound);
        };
        // 切换了地址空间， 需要切换token
        let page_table_token = if self.pid.as_u64() == KERNEL_EXECUTOR_ID {
            0
        } else {
            self.memory_set.lock().await.page_table_token()
        };
        if page_table_token != 0 {
            unsafe {
                axhal::arch::write_page_table_root0(page_table_token.into());
            };
            // 清空用户堆，重置堆顶
        }
        // 重置用户堆
        self.set_heap_bottom(heap_bottom.as_usize() as u64);
        self.set_heap_top(heap_bottom.as_usize() as u64);
        // // // 重置robust list
        // self.robust_list.lock().clear();
        // self.robust_list
        //     .lock()
        //     .insert(current_task.id().as_u64(), FutexRobustList::default());

        {
            use axhal::mem::virt_to_phys;
            use axhal::paging::MappingFlags;
            // 重置信号处理模块
            // 此时只会留下一个线程
            self.signal_modules.lock().await.clear();
            self.signal_modules
                .lock()
                .await
                .insert(current_task.id().as_u64(), SignalModule::init_signal(None));

            // 生成信号跳板
            let signal_trampoline_vaddr: VirtAddr = (axconfig::SIGNAL_TRAMPOLINE).into();
            let signal_trampoline_paddr = virt_to_phys((start_signal_trampoline as usize).into());
            let memory_set_wrapper = self.memory_set.lock();
            let mut memory_set = memory_set_wrapper.await;
            if memory_set.query(signal_trampoline_vaddr).is_err() {
                let _ = memory_set.map_page_without_alloc(
                    signal_trampoline_vaddr,
                    signal_trampoline_paddr,
                    MappingFlags::READ
                        | MappingFlags::EXECUTE
                        | MappingFlags::USER
                        | MappingFlags::WRITE,
                );
            }
            drop(memory_set);
        }

        // // user_stack_top = user_stack_top / PAGE_SIZE_4K * PAGE_SIZE_4K;
        // let new_trap_frame =
        //     TrapFrame::app_init_context(entry.as_usize(), user_stack_bottom.as_usize());
        // write_trapframe_to_kstack(
        //     current_task.get_kernel_stack_top().unwrap(),
        //     &new_trap_frame,
        // );
        let utrap_frame = current_task.utrap_frame().unwrap();
        *utrap_frame = TrapFrame::init_user_context(entry.as_usize(), user_stack_bottom.as_usize());

        // release vfork for parent process
        {
            let pid2pc = PID2PC.lock().await;
            let parent_process = pid2pc.get(&self.get_parent()).unwrap();
            parent_process.set_vfork_block(false).await;
            drop(pid2pc);
        }
        Ok(())
    }
}
