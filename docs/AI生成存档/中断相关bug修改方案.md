分析完成。我在本地用与 output.txt 相同的构建（coroutine_test 8-15 22:52 的 .bin、vsched2 的 .so、-smp 4）复现了问题，并用 QEMU 的中断日志、
  指令日志和 monitor 寄存器快照拿到了挂死现场。下面是完整的根因分析。

• Proposed Plan


  # coroutine_test 多核收尾挂死：根因分析

  ## 现象（已复现并确认）

  - SMP=4 LOG=warn make run 运行 coroutine_test 随机挂死或通过（我观察到一次完整 test passed!，多次挂死），确认是时序相关的竞态，不是确定性
    死锁。

  - 挂死时最后的日志模式（与 output.txt 一致）：某个任务退出时 Task(N): notify waker for exit，随后 wake Task(7, "main"): Blocked ->
    Runnable；此后 main 不再有 res 0、join Task(...) 等任何输出，QEMU 只能被杀掉。

  - 用 -d int 抓到的中断日志显示：挂死后有持续的 IPI 风暴——睡眠核反复被 s_software 中断从 WFI 唤醒，进入 slow_path_entry（先执行 SBI
    SET_TIMER），回到调度器发现没有可运行任务，再次 WFI，又被唤醒。

  - -d in_asm 指令日志显示挂死终态是多个核卡在自旋锁上：全局分配器锁（set_stack_ctx/_stack_ctx绪队列/sources）。
  - 关闭时这套 WFI/IS_SLEEP不出现该问题——与你的lostw) 在任务处于 Block（：[run_coroutineloop.rs为 Ready（任务状态变成是 Waked；随后 /home/
    rosy/桌面/vsched2/src/main_loop.rs:464 对 Waked 走 _state => {} 分支——既不入队也不做任何事。

  - 结果：该任务状态是“就绪”却不在就绪队列里；之后再对它 wakeup_task 时 Runable => () 也是空操作，任务被永久丢出调度。失败运行的日志中能看
    到 wake Task(N): Blocking -> Waked 事件。这能解释“部分任务永远执行不完”、以及 main 等待的任务始终不结束导致的挂死变体。

  ### 缺陷 B：收尾阶段（最后一个任务唤醒 main、其它核全部睡眠）的唤醒/取任务路径形成活锁

  - 最后一刻 CPU 1 在 Task::dealloc 里唤醒 main：main 从 Blocked -> Runnable，push_task 成功把 main 放进内核调度器就绪队列（有 assert!
    (res)，若失败会 panic，因此 main 确实在队列里）。

  - /home/rosy/桌面/vsched2/src/schedule/scheduler.rs:282 只向 IS_SLEEPING 的核中第一个发一个 IPI，且没有任何“任务最终被某个核取走”的确认/
    重试机制；同时 IPI 唤醒路径（trap_entry 的 scheduler_waiting 分支 → thread_stack() 取走 wait_context 的栈 → trap 处理 → kschedule）本身
    非常重：每个中断在 slow_path_entry 分配 Box<TrapFrame>、在恢复路径释放，全部竞争全局分配器锁，还叠加任务状态锁、就绪队列锁竞争。

  - 实测终态：IPI 风暴 + 多核在分配器/状态/队列自旋锁上互等，没有任何核再运行 main（没有 res 0）。这是一次“多核同时陷入中断/trap/SBI 处理 +
    热锁竞争”的活锁，而不是单点逻辑错误，因此偶尔也能通过（竞争没有触发时）。

  - 另外，中断风暴本身会自我维持：被中断的任务经 push_prev_task(Ready) 重新入队，入队又触发新的 IPI，形成闭环。

  ## 修复方向

  1. 修 Waked 丢失唤醒：在 run_coroutine/thread_entry_phase2 的 Pending 分支中，把 Waked 当作 Ready 处理——先 match_set_state 置为 Ready 后
     调用 push_prev_task(TaskState::Ready)（或给 push_prev_task 增加 Waked 分支，行为等同 Ready）。同时建议在 wakeup_task 对 Waked 增加日志
     以便验证。

  2. 收尾唤醒可靠性：
      - push_task/push_trap 后改为向所有 IS_SLEEPING=true 的核发 IPI（或至少循环全部核），并考虑在 push 后立即触发本核重新调度；
      - 让 wait_for_runnable_task 的“第二次检查 + WFI”与 IPI 之间增加可观测性（例如记录每次 IPI 唤醒后取到的任务），先确认风暴的源头再收
        敛；

      - 优化中断入口/恢复路径的分配：缓存复用 TrapFrame Box（或栈上复用），避免每个中断都抢全局分配器锁。

  3. 需要同步检查 wait_context 的栈/上下文管理：thread_stack() 在每次 IPI 唤醒时把 kstack 从 wait_context 取走，多次睡眠-唤醒周期后
     StackCtx 的状态（kstack/trap_frame 所有权）需要保持一致，这是 IPI 路径最容易出错的地方。

  ## 验证方法

  - 修复缺陷 A 后：反复运行 SMP=4 LOG=warn make run（建议 ≥50 次）不再出现任务缺失/挂死；观察日志中不再有任务被 Blocking -> Waked 后消失。
  - 修复缺陷 B 后：挂死现场应不再出现收尾 IPI 风暴；在 SMP=2/4/8 与 irq 开/关下分别跑 coroutine_test 与 perfomance_test，全部能打印 test
    passed!。

  - 回归：output.txt 对应场景（100 任务 + main join）与既有其它 app（thread_test 等）保持可运行。

  - 分析基于 output.txt 对应的构建：coroutine_test 二进制（8-15 22:52）与 vdso_output/libvsched2.so（8-15 20:31，包含未提交的
    trap_wait_queue 改动），SMP=4、async_std/smp。

  - 问题定位在 vsched2 调度器与任务状态机，而不是应用代码本身（apps/coroutine_test/src/main.rs 只是暴露了该竞态）。

## 点评

对任务状态的分析不正确，taskctx::TaskState::Waked已经对应到了libvsched2::TaskState::Ready。

是否出现了IPI风暴，需要验证。原本不应出现。可能是bug的原因。

验证：确实出现了。还不清楚原因。