# 卡死bug原因分析

分析完成，我没有修改任何代码。

最可能的根因是 **异步等待队列存在“丢失唤醒”（lost wakeup）竞态**，而不是应用本身或 Mutex 使用方式的问题。

关键位置：

- `modules/sync/src/mutex.rs` 的 `force_unlock()`：先执行 `owner_task.swap(0)` 释放锁，再调用 `wq.notify_one()`。
- `modules/sync/src/wait_queue.rs` 的 `WaitUntilFuture::poll()`：先检查条件 `_condition()`（即 `!lock.is_locked()`），如果仍被锁住，才关中断并调用 `prepare_to_wait()` 把自己加入等待队列。

竞态时序如下：

1. CPU A 上的任务尝试获取 Mutex，CAS 失败。
2. CPU A 进入 `WaitUntilFuture::poll()`，检查条件时锁仍被持有，于是准备把自己加入等待队列。
3. 在它真正 `prepare_to_wait()` 之前，CPU B 上的任务释放了同一把锁：`owner_task` 被清 0，并调用 `notify_one()`。
4. 此时等待队列还是空的，`notify_one()` 没有唤醒任何任务。
5. CPU A 随后才把自己加入等待队列并睡眠。
6. 这次“释放锁”对应的唤醒已经丢失；如果之后没有其他任务再释放锁，CPU A 会永久阻塞。

在 `CPU_NUM=4 SMP=4` 下，100 个任务同时在多核上争抢同一个 Mutex，上述检查与入队之间的窗口被并发放大的概率显著增加。一旦某个子任务因该竞态永久阻塞，`main` 的 `join().await` 就会一直等不到它，表现为整体卡死，`test passed!` 永远不出现。

另外还有两个次要可疑点，但都不是首要根因：

- `WaitUntilFuture::poll()` 每次 poll 都新建一个 `Arc<WaitWakerNode>`，唤醒后 `remove(&waker_node)` 删除的并不是之前真正入队的那个节点，可能残留陈旧等待节点，造成重复/虚假唤醒。
- vsched2 当前 `Scheduler::push_task()` / `push_trap()` 主动 IPI 唤醒逻辑被注释掉，睡眠核依赖时钟中断；这会让“唤醒延迟”变大，但不会单独造成上述丢失唤醒。

结论：当前偶发卡死的核心原因应定位到 **异步 Mutex/WaitQueue 的 check-then-sleep 与 unlock-then-notify 之间缺少同一把锁保护的 lost wakeup 竞态**。修复方向通常是让“检查条件 + 入队”和“清 owner + notify”在同一把队列锁的保护下完成，或引入唤醒序号/世代计数来检测丢失唤醒。