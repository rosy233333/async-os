#![no_std]
#![no_main]

use async_std::println;
use vdso_lib::*;

#[async_std::async_main]
async fn main() -> isize {
    println!("vdso scheduler test (kernel):");
    init();

    // 创建内核调度器
    assert!(add_scheduler(0, None)); 
    // // 模拟创建内核任务
    // assert!(add_task(0, 0x8003, 3) == true);
    // assert!(add_task(0, 0x8002, 2) == true);
    // assert!(add_task(0, 0x8001, 1) == true);
    // assert!(add_task(0, 0x8000, 0) == true);
    // // 模拟取出内核任务并运行，在内核任务中创建用户任务
    // assert!(pick_next_task(0) == Some(0x8000));
    // assert!(clear_current(0) == false);
    // assert!(pick_next_task(0) == Some(0x8001)); // 此时，内核调度器剩余的最高优先级为2
    // assert!(add_scheduler(0x80000000, Some((0x8001, 0)))); // 创建用户调度器后，内核任务的优先级与用户调度器绑定
    // // 此处模拟进入用户态并操作用户调度器
    // assert!(add_task(0x80000000, 0x8800, 0) == true);
    // assert!(add_task(0x80000000, 0x8801, 1) == false);
    // assert!(add_task(0x80000000, 0x8802, 2) == false);
    // assert!(add_task(0x80000000, 0x8803, 3) == false);
    // assert!(pick_next_task(0x80000000) == Some(0x8800));
    // assert!(clear_current(0x80000000) == false); // 用户调度器优先级变为1
    // assert!(pick_next_task(0x80000000) == Some(0x8801));
    // assert!(clear_current(0x80000000) == false); // 用户调度器优先级变为2
    // assert!(pick_next_task(0x80000000) == Some(0x8802));
    // assert!(clear_current(0x80000000) == true); // 用户调度器优先级变为3，触发内核调度器重新调度
    // // 此处模拟陷入内核态并重新调度
    // assert!(pick_next_task(0) == Some(0x8002));
    // assert!(clear_current(0) == false);

    // // 测试删除调度器
    // assert!(delete_scheduler(0x80000000));
    // assert!(delete_scheduler(0));

    println!("test ok!");
    0
}