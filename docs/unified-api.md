# unified-api

目标：设计一套能够在 async 和 non-async 环境下使用的 API，能够同时支持以线程、或者协程为任务单元的切换

这里的设计围绕着与任务调度相关的接口（`yield_now` 等）以及内核中使用的睡眠锁（`Mutex`）进行展开，
同理可以推广至其他需要进行统一的内核子系统中。

## APIs about Task scheduling

### yield

```rust
pub fn yield_now() {
    ......
}
```

上述为原本的 `yield_now` 接口的定义，它虽然可以同时在 async 和 non-async 环境下使用，但在这两种环境下，任务都将以线程的形式进行切换，且不能栈复用。

为了保证在 async 环境下，能够利用协程的优势，因此将接口定义为如下形式，增加了 `YieldFuture` 返回值：

```rust
pub fn yield_now() -> YieldFuture {
    #[cfg(feature = "thread")]
    thread_yield();
    YieldFuture::new()
}

#[derive(Debug)]
pub struct YieldFuture{
    _has_polled: bool, 
    _irq_state: <NoPreemptIrqSave as BaseGuard>::State
}

impl YieldFuture {
    pub fn new() -> Self {
        // 这里获取中断状态，并且关中断
        #[cfg(feature = "thread")]
        let _irq_state = Default::default();
        #[cfg(not(feature = "thread"))]
        let _irq_state = NoPreemptIrqSave::acquire();
        Self{ _has_polled: false, _irq_state }
    }
}

impl Future for YieldFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[cfg(feature = "thread")]
        return Poll::Ready(());
        #[cfg(not(feature = "thread"))]
        {
            if self._has_polled {
                // 恢复原来的中断状态
                NoPreemptIrqSave::release(self._irq_state);
                Poll::Ready(())
            } else {
                self.get_mut()._has_polled = true;
                _cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}
```

根据使用的环境（async、non-async）以及使用形式（await、non-await），存在以下四种组合

|           | non-await | await |
|-----------|-----------|-------|
| non-async | √         | ×     |
| async     | √         | √     |

接口使用

1. 在 async 环境下，建议使用 `yield_now().await`，若使用 `yield_now()`，则必须使能 `thread` feature，否则不会让权；
2. 在 non-async 环境下，只能使用 `yield_now()`，同时必须使能 `thread` feature

**注意：**`YieldFuture` 的 `poll` 函数实现与 `thread` feature 相关，这是为了避免在 async 环境下，同时使能了 `thread` feature，并且使用 `yield_now().await`，这种方式能够避免发生两次让权的情况。

## API about Mutex

这里直接列出统一后的接口

```rust
pub struct Mutex<T: ?Sized> {
    wq: WaitQueue,
    owner_task: AtomicUsize,
    data: UnsafeCell<T>,
}

pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
    data: Option<*mut T>,
}

pub fn lock(&self) -> MutexGuard<T> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "thread")] {
            let curr = current_task();
            let waker = curr.waker();
            let current_task = waker.as_raw().data() as usize;
            loop {
                match self.owner_task.compare_exchange_weak(
                    0,
                    current_task,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(owner_task) => {
                        assert_ne!(
                            owner_task, current_task,
                            "{} tried to acquire mutex it already owns.",
                            curr.id_name(),
                        );
                        self.wq.wait_until(|| !self.is_locked());
                    }
                }
            }
            return MutexGuard {
                lock: self,
                data: Some(self.data.get()),
            };
        } else if #[cfg(not(feature = "thread"))] {
            return MutexGuard {
                lock: self,
                data: None,
            }
        }
    }
}

/// 这里要实现所有权的转移，否则会导致连续两次 drop，重复释放锁
impl<'a, T: ?Sized + 'a> Future for MutexGuard<'a, T> {
    type Output = Self;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { lock, data } = self.get_mut();
        cfg_if::cfg_if! {
            if #[cfg(feature = "thread")] {
                assert!(data.is_some());
                Poll::Ready(MutexGuard {
                    lock,
                    data: data.take(),
                })
            } else if #[cfg(not(feature = "thread"))] {
                if data.is_none() {
                    let curr = current_task();
                    let current_task = _cx.waker().as_raw().data() as usize;
                    match lock.owner_task.compare_exchange_weak(
                        0,
                        current_task,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            Poll::Ready(MutexGuard {
                                lock,
                                data: Some(lock.data.get()),
                            })
                        },
                        Err(owner_task) => {
                            assert_ne!(
                                owner_task, current_task,
                                "{} tried to acquire mutex it already owns.",
                                curr.id_name(),
                            );
                            // 当前线程让权，并将 cx 注册到等待队列上
                            let a = Pin::new(&mut lock.wq.wait_until(|| !lock.is_locked())).poll(_cx);
                            // 进入这个分支一定是属于 Poll::Pending 的情况
                            assert_eq!(&a, &Poll::Pending);
                            Poll::Pending
                        }
                    }
                } else {
                    Poll::Ready(MutexGuard {
                        lock,
                        data: data.clone(),
                    })
                }
            }
        }
    }
}
```

与原有的接口的不同之处在于 `MutexGuard` 中的数据使用了 `Option<T>` 类型来表示，同时对于 `lock` 方法的实现也与 `thread` feature 相关。

当使能了 `thread` feature 时，`lock` 方法直接使用线程相关的接口获取到内部的数据，当存在多个任务同时获取锁时，后尝试获取锁的任务将以线程的形式阻塞在这个锁上。

当没有使能 `thread` feature 时，`lock` 方法不会获取到内部的数据（`data` 为 `None`），此时需要使用 `await` 才能实际获取到内部的数据结构。

接口使用：

1. 在 async 环境下，建议使用 `lock().await`，若使用 `lock()`，则必须使能 `thread` feature，否则不会获取实际的数据，在运行时会 `panic`；
2. 在 non-async 环境下，只能使用 `lock()`，同时必须使能 `thread` feature

**注意：**`MutexGuard` 的 `poll` 函数实现与 `thread` feature 相关，这是为了避免在 async 环境下，同时使能了 `thread` feature，并且使用 `lock().await`，并且在 `poll` 函数中要实现内部的数据所有权的转移，否则会出现连续两次释放锁的情况。

## APIs about others

根据这两种情况，我们可以总结出将接口统一需要的一般原则：

1. 函数名和参数可以保持不变，在必要时需要增加生命周期注解；
2. 对于没有返回值的接口，增加 Future 返回值。若在 non-async 环境中使用，则在接口内部实现相关的逻辑，Future 内的实现直接返回 `Poll::Ready(())`；若在 async 环境下使用，则接口仅仅返回 Future，在 Future 内再实现相关的逻辑，并通过 `.await` 驱动；
3. 对于具有返回值的接口，将返回值用 `Option<T>` 封装，并封装在 Future 对象中。当在 non-async 环境中使用，接口内部直接处理，返回值内部的数据为 `Some(T)`，Future 对象可直接解引用为 `T`，`.await` 操作将直接返回 `Poll::Ready(T)`；在 async 环境下使用，接口直接返回 Future 对象，在 Future 对象中实现相关的逻辑，并通过 `.await` 驱动。**注意：对于返回值需要 drop 的类型，在 Future 的内部逻辑中需要实现所有权转移，否则就会出现两次释放**
   1. 对于返回值为自定义类型，参考 `Mutex` 的实现
   2. 对于返回值为 `bool`、`isize` 等基本类型时，则按照如下方式
    ```rust
    /// 原始接口
    pub fn sys_read(fd: usize, buf: &mut [u8]) -> isize {
        // non-async 实现逻辑
        ......
    }

    /// 统一接口
    pub fn sys_read<'a>(fs: usize, buf: &'a mut [u8]) -> SyscallRes {
        #[cfg(feature = "thread")]
        {
            // non-async 逻辑
            let res = xx usize;
            return SyscallRes { 
                res: Some(res), 
                args: [fd, buf.ptr() as _, buf.len() as _, 0, 0, 0] 
            };
        }
        #[cfg(not(feature = "thread"))]
        return SyscallRes { 
            res: None, 
            args: [fd, buf.ptr() as _, buf.len() as _, 0, 0, 0] 
        };
    }

    pub struct SyscallRes {
        res: Option<isize>,
        args: [usize; 6]
    }

    /// 可直接对 SyscallRes 解引用，获取到返回值
    impl Deref for SyscallRes {
        type Target = isize;
        fn deref() -> &Self::Target {
            self.res.as_ref().unwrap()
        }
    }

    impl Future for SyscallRes {
        type Output = isize;
        
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            #[cfg(feature = "thread")]
            {
                // 必要时需要实现 res 的所有权转移，避免重复释放
                return Poll::Ready(self.res.unwrap());
            }
            #[cfg(not(feature = "thread"))]
            {
                // 协程逻辑
                Poll::Ready(res)
            }
        }
    }
    ```
    使用这种设计能够保证在 async 和 non-async 环境下，可以使用同一个接口，在 async 环境下，则会在后面使用 `.await` 进行驱动。但在非 async 环境下需要增加一些额外的解引用的操作。

关于这种接口的使用案例，可以见 [sync/src/mutex.rs](../modules/sync/src/mutex.rs) 以及 [sync/src/wait_queue.rs](../modules/sync/src/wait_queue.rs)
