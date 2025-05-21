use core::{alloc::Layout, ptr::NonNull};
use queue::{AtomicCell, LockFreeQueue};

/// 任务使用的运行栈
#[derive(Debug)]
pub struct RunningStack {
    ptr: NonNull<u8>,
    layout: Layout,
    is_init: bool,
}

impl RunningStack {
    pub fn new_init(curr_boot_stack: *mut u8) -> Self {
        let layout = Layout::from_size_align(axconfig::TASK_STACK_SIZE, 16).unwrap();
        Self {
            ptr: NonNull::new(curr_boot_stack).unwrap(),
            layout,
            is_init: true,
        }
    }

    pub fn alloc(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 16).unwrap();
        Self {
            ptr: NonNull::new(unsafe { alloc::alloc::alloc(layout) }).unwrap(),
            layout,
            is_init: false,
        }
    }

    pub fn top(&self) -> usize {
        unsafe { self.ptr.as_ptr().add(self.layout.size()) as _ }
    }

    pub fn down(&self) -> usize {
        self.ptr.as_ptr() as _
    }
}

impl Drop for RunningStack {
    fn drop(&mut self) {
        if !self.is_init {
            unsafe { alloc::alloc::dealloc(self.ptr.as_ptr(), self.layout) }
        }
    }
}

/// 运行栈池，需要使用位置无关的无锁链表，不要求支持 MPMC，
/// 因为是局部的，每个处理器对应一个栈池，只需要 SPSC 即可
/// 若任务占用了某个运行栈，因为处理器只有一个，所以同一时刻只会有一个线程使用，
/// 若由于负载均衡机制导致在处理器之间迁移，栈的局部性与任务中的 CPU 亲和掩码相关
/// 这里能够保证是正确的
pub struct StackPool {
    free_stacks: LockFreeQueue<RunningStack>,
    current: AtomicCell<Option<RunningStack>>,
}

impl StackPool {
    /// Creates a new empty stack pool.
    pub fn new() -> Self {
        Self {
            free_stacks: LockFreeQueue::new(),
            current: AtomicCell::new(None),
        }
    }

    /// 初始化运行栈，
    /// 因为初始化使用的栈是声明的 static 变量，因此需要将栈的指针以参数的形式传递过来
    pub fn init(&self, curr_boot_stack: *mut u8) {
        self.current
            .store(Some(RunningStack::new_init(curr_boot_stack)));
    }

    /// 从空闲运行栈池中取出一个运行栈，若没有则从堆中分配一个新的运行栈
    fn alloc(&self) -> RunningStack {
        self.free_stacks.pop().unwrap_or_else(|| {
            let stack = RunningStack::alloc(axconfig::TASK_STACK_SIZE);
            stack
        })
    }

    /// 从处理器中取出当前的运行栈
    pub fn pick_current_stack(&self) -> RunningStack {
        let curr = self.current.take();
        assert!(curr.is_some());
        let new_stack = self.alloc();
        self.current.store(Some(new_stack));
        curr.unwrap()
    }

    /// 获取当前运行栈的引用
    pub fn current_stack(&self) -> &RunningStack {
        let curr_ptr = self.current.as_ptr();
        unsafe { (*curr_ptr).as_ref().expect("current stack is Some") }
    }

    /// 设置当前运行栈
    pub fn set_current_stack(&self, stack: RunningStack) {
        let curr = self.current.take();
        self.current.store(Some(stack));
        self.free_stacks.push(curr.unwrap());
    }
}
