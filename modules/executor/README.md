# Executor

Executor 不仅仅提供了任务管理功能，而且集成了与进程相关的抽象，因此其内部包含了调度器、地址空间抽象、文件的抽象等。Executor 对应于系统中的进程。

异步 IPC 将会转化为不同的 Executor 之间的通信。

这种模式会导致任务调度需要进行修改，目前还没有思考清楚。