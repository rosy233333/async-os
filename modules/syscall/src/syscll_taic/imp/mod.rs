use crate::{SyscallError, SyscallResult};
use alloc::format;
use alloc::{boxed::Box, collections::BTreeMap, sync::Arc};
use axhal::mem::virt_to_phys;
use axhal::{mem::PAGE_SIZE_4K, paging::MappingFlags};
use heapless::mpmc::MpMcQueue;
use process::{current_process, yield_now};
use taic_driver::{LocalQueue, Taic};
type SyscallItemQueue = MpMcQueue<SyscallItem, 8>;

const TAIC_BASE: usize = axconfig::PHYS_VIRT_OFFSET + axconfig::MMIO_REGIONS[1].0;
const LQ_NUM: usize = 2;
const TAIC: Taic = Taic::new(TAIC_BASE, LQ_NUM);
use sync::Mutex;
pub static LQS: Mutex<BTreeMap<(usize, usize), LocalQueue>> = Mutex::new(BTreeMap::new());

/// 获取控制器的资源
pub async fn syscall_get_taic() -> SyscallResult {
    let current_process = current_process().await;
    let pid = current_process.pid() as usize;
    if let Some(lq) = TAIC.alloc_lq(1, pid) {
        let lq_pbase = virt_to_phys((lq.regs() as *const _ as usize).into());
        let mut memory_set = current_process.memory_set.lock().await;
        // 这里不能直接使用 max_va，因为 max_va 为 0x4000_0000，已经被用于映射信号页
        let lq_vbase = memory_set.find_free_area(0.into(), PAGE_SIZE_4K).unwrap();
        let _ = memory_set
            .map_attach_page_without_alloc(
                lq_vbase,
                lq_pbase,
                1,
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
            )
            .await;
        LQS.lock().await.insert((1, pid as _), lq);
        Ok(lq_vbase.as_usize() as isize)
    } else {
        Err(SyscallError::ENOMEM)
    }
}

/// 使用控制器进行异步系统调用时，初始化
/// 1. 分配两块内存区域，用于用户态和内核态之间进行通信，将起始地址返回给用户态
/// 2. 初始化内核态运行的系统调用处理任务 ksyscall，并将其注册为接收方，将 ksyscall 的 id 返回给用户态
pub async fn syscall_init_async_batch(_waker: usize, res_ptr: usize) -> SyscallResult {
    let current_process = current_process().await;
    let mut memory_set = current_process.memory_set.lock().await;
    // 初始化内核与用户态通信的系统调用页面
    let syscall_recv_page_start = memory_set.find_free_area(0.into(), PAGE_SIZE_4K).unwrap();
    let _ = memory_set
        .new_region(
            syscall_recv_page_start,
            PAGE_SIZE_4K,
            false,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
            Some(&[]),
            None,
        )
        .await;
    let syscall_send_page_start = memory_set.find_free_area(0.into(), PAGE_SIZE_4K).unwrap();
    let _ = memory_set
        .new_region(
            syscall_send_page_start,
            PAGE_SIZE_4K,
            false,
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
            Some(&[]),
            None,
        )
        .await;
    // #[cfg(target_arch = "riscv64")]
    // unsafe {
    //     riscv::register::sstatus::set_sum();
    // }
    // axhal::arch::flush_tlb(None);
    // 初始化该进程在内核中处理系统调用的任务
    let recv_syscall_items =
        unsafe { &mut *(syscall_recv_page_start.as_usize() as *mut SyscallItemQueue) };
    *recv_syscall_items = SyscallItemQueue::new();
    let send_syscall_items =
        unsafe { &mut *(syscall_send_page_start.as_usize() as *mut SyscallItemQueue) };
    *send_syscall_items = SyscallItemQueue::new();
    // let paddr = MMIO_REGIONS[1].0;
    // let ktaic = Taic::new(axconfig::PHYS_VIRT_OFFSET + paddr);
    let fut = Box::pin(async move {
        loop {
            if let Some(syscall_item) = recv_syscall_items.dequeue() {
                // let meta = waker as *const taic_driver::TaskMeta<TaskInner>;
                // let send_task_id = meta.into();
                // ktaic.send_intr(*OS_ID, *PROCESS_ID, send_task_id);
                // continue;
                let ret_ptr = syscall_item.ret_ptr;
                let _waker = syscall_item.waker;
                let res = crate::trap::handle_syscall(syscall_item.id, syscall_item.args).await;
                // 将结果写回到用户态 SyscallFuture 的 res 中
                unsafe {
                    let ret = ret_ptr as *mut Option<Result<usize, syscalls::Errno>>;
                    (*ret).replace(syscalls::Errno::from_ret(res as _));
                }
                // debug!("handle {:#X?}", syscall_item);
                let _ = send_syscall_items.enqueue(syscall_item).unwrap();
                // 这里不需要增加用户态任务唤醒的逻辑，由用户态的 dispatcher 进行唤醒
            } else {
                // debug!("run ksyscall task");
                yield_now().await;
            }
        }
    });
    drop(memory_set);
    debug!("syscall_init_async_syscall new ktask");
    let ktask = current_process
        .new_ktask(
            format!("async_syscall_handler {}", current_process.pid()),
            fut,
        )
        .await;
    // 这个内核任务直接进入阻塞状态，需要通过 taic 来唤醒
    ktask.set_state(process::TaskState::Blocked);
    // 将这个任务注册为系统调用处理流程，注册为接收方，获取内核的调度器
    use process::KERNEL_SCHEDULER;
    let handler = Arc::into_raw(ktask) as *const _ as usize;
    let pid = current_process.pid() as usize;
    KERNEL_SCHEDULER.lock().register_receiver(1, pid, handler);

    // 注册用户态任务为发送方
    let lqs = LQS.lock().await;
    let ulq = lqs.get(&(1, pid)).unwrap();
    ulq.register_sender(1, 0);
    // let utaic = Taic::new(axconfig::PHYS_VIRT_OFFSET + paddr + 0x1000);
    // let recv_os_id = ktaic.current::<TaskInner>();
    // // ktaic.register_sender(recv_task_id, *OS_ID, *PROCESS_ID, send_task_id);
    // utaic.register_sender(
    //     send_task_id,
    //     recv_os_id,
    //     unsafe { TaskId::virt(0) },
    //     recv_task_id,
    // );
    // utaic.register_receiver(
    //     send_task_id,
    //     recv_os_id,
    //     unsafe { TaskId::virt(0) },
    //     recv_task_id,
    // );
    let res_ptr = unsafe { &mut *(res_ptr as *mut AsyncBatchSyscallResult) };
    *res_ptr = AsyncBatchSyscallResult {
        send_channel: syscall_send_page_start.as_usize(),
        recv_channel: syscall_recv_page_start.as_usize(),
        recv_os_id: 1,
        recv_process_id: 0,
    };
    Ok(0)
}

#[repr(C, align(128))]
#[derive(Debug)]
struct SyscallItem {
    id: usize,
    args: [usize; 6],
    ret_ptr: usize,
    waker: usize,
}

#[allow(unused)]
#[derive(Default)]
pub struct AsyncBatchSyscallResult {
    pub send_channel: usize,
    pub recv_channel: usize,
    pub recv_os_id: usize,
    pub recv_process_id: usize,
}
