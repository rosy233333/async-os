# state

## 任务状态转移模型

与任务的类型是线程、协程无关，任务状态转移涉及到往任务队列中取放。

五种基本任务状态：

1. 创建
2. 就绪
3. 运行
4. 阻塞
5. 退出

任务状态之间的转移

- [x] `创建 -> 就绪`：new_task，直接调用 add_task 向就绪队列中放任务，这个状态变化也只会在一个核上发生；
- [x] `就绪 -> 运行`：pick_next_task，从就绪队列中取出一个任务。若就绪队列中，这个任务只存在一个标签，这个状态变化也只会在一个核上发生；
- [x] `运行 -> 就绪`：返回 pending 后，任务状态仍然是 Running，调用 put_prev_task 将其放入到就绪队列中。若只有一个核在运行这个任务，这个状态变化也只会在一个核上发生；
- [x] `运行 -> 阻塞`：返回 pending 后，任务状态处于 Blocking，将其设置为 Blocked，不对就绪队列进行操作。若只有一个核在运行这个任务，这个状态变化只会发生在一个核上；
- [x] `阻塞 -> 就绪`：必须等待任务状态完全处于 Blocked 后，才调用 add_task 将其放入到就绪队列中，对应 wakeup_task 函数。一个处于阻塞状态的任务，只能被其他的任务或者中断唤醒，必须确保这个任务没有在核上运行。
- [x] `运行 -> 退出`：若只有一个核在运行这个任务，这个状态变化也只会在一个核上发生；

因此，关于任务状态转移与任务队列的操作主要在于 `阻塞 -> 就绪` 这个状态变化，被唤醒的任务的状态如何转移。

但在实际的代码中，可能会存在中间状态。因此在实际代码中对应于以下几种状态，

1. Running：正在 CPU 上运行的任务
2. Runable：处于就绪队列中的任务
3. Blocking：被阻塞了，但还未完全让出 CPU
4. Blocked：处于阻塞队列中
5. Waked：正处于 Blocking 状态的任务被其他核上执行的任务唤醒后，进入这个状态，处于这个状态的任务不会放弃 CPU，而是继续运行
6. Exited：已经结束的任务，需要注意 memory_set 被释放后，页表失效，但任务还未完全让出 CPU 导致的页错误

wakeup_task 对应了 `阻塞 -> 就绪` 这个状态变化，实际代码如下：

```rust
match **state {
    // 任务正在运行，且没有让权，不必唤醒
    // 可能不止一个其他的任务在唤醒这个任务，因此被唤醒的任务可能是处于 Running 状态的
    TaskState::Running => (),
    // 任务准备让权，但没有让权，还在核上运行，但已经被其他核唤醒，此时只需要修改其状态即可
    // 后续的处理由正在核上运行的自己来决定
    TaskState::Blocking => **state = TaskState::Waked,
    // 任务不在运行，但其状态处于就绪状态，意味着任务已经在就绪队列中，不需要再向其中添加任务
    TaskState::Runable => (),
    // 任务不在运行，已经让权结束，不在核上运行，就绪队列中也不存在，需要唤醒
    // 只有处于 Blocked 状态的任务才能被唤醒，这时候才会拿到任务的 Arc 指针
    TaskState::Blocked => {
        **state = TaskState::Runable;
        let task_ref = unsafe { Arc::from_raw(task_ptr) };
        task.scheduler.lock().lock().add_task(task_ref);
    },
    TaskState::Waked => panic!("cannot wakeup Waked {}", task.id_name()),
    // 无法唤醒已经退出的任务
    TaskState::Exited => panic!("cannot wakeup Exited {}", task.id_name()),
};
```