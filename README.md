# async-os

这个仓库借鉴了 [Arceos](https://github.com/arceos-org/arceos) 和 [Starry](https://github.com/Starry-OS/Starry) 的实现，尽可能的利用这两个仓库中已有的 crate 实现。在此基础上来使用协程来构建异步内核和用户态程序。

这个内核基于共享调度器和 Rust 异步协程，并且将调度实体的粒度对齐到基于执行流的任务模型上，通过跳板页来实现跨不同地址空间、特权级的任务之间的切换。

要使用外部设备的话，必须使能 `paging` feature，因为在没有使能时，没有对低位的设备的 MMIO 地址进行映射，只映射了内核的地址
