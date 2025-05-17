use crate::{Executor, ExecutorRef};
use alloc::sync::Arc;
use core::{mem::ManuallyDrop, ops::Deref};

#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
#[inline]
fn local_irq_save_and_disable() -> usize {
    const SIE_BIT: usize = 1 << 1;
    let flags: usize;
    // clear the `SIE` bit, and return the old CSR
    unsafe { core::arch::asm!("csrrc {}, sstatus, {}", out(reg) flags, const SIE_BIT) };
    flags & SIE_BIT
}

#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
fn local_irq_restore(flags: usize) {
    // restore the `SIE` bit
    unsafe { core::arch::asm!("csrrs x0, sstatus, {}", in(reg) flags) };
}

#[cfg_attr(not(target_os = "macos"), link_section = ".percpu")]
static mut __PERCPU_CURRENT_EXECUTOR_PTR: usize = 0;

#[allow(non_camel_case_types)]
/// Wrapper struct for the per-CPU data [stringify! (CURRENT_EXECUTOR_PTR)]
struct CURRENT_EXECUTOR_PTR_WRAPPER {}

#[allow(unused)]
static CURRENT_EXECUTOR_PTR: CURRENT_EXECUTOR_PTR_WRAPPER = CURRENT_EXECUTOR_PTR_WRAPPER {};

#[allow(dead_code)]
impl CURRENT_EXECUTOR_PTR_WRAPPER {
    /// Returns the offset relative to the per-CPU data area base on the current CPU.
    fn offset(&self) -> usize {
        let value: usize;
        unsafe {
            #[cfg(target_arch = "x86_64")]
            core::arch::asm!(
                "movabs {0}, offset {VAR}",
                out(reg) value,
                VAR = sym __PERCPU_CURRENT_EXECUTOR_PTR,
            );

            #[cfg(target_arch = "aarch64")]
            core::arch::asm!(
                "movz {0}, #:abs_g0_nc:{VAR}",
                out(reg) value,
                VAR = sym __PERCPU_CURRENT_EXECUTOR_PTR,
            );

            #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
            core::arch::asm!(
                "lui {0}, %hi({VAR})",
                "addi {0}, {0}, %lo({VAR})",
                out(reg) value,
                VAR = sym __PERCPU_CURRENT_EXECUTOR_PTR,
            );
        }
        value
    }
    #[inline]
    /// Returns the raw pointer of this per-CPU data on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    pub unsafe fn current_ptr(&self) -> *const usize {
        #[cfg(not(target_os = "macos"))]
        {
            let base: usize;
            #[cfg(target_arch = "x86_64")]
            {
                core::arch::asm!(
                    "mov {0}, gs:[offset __PERCPU_SELF_PTR]",
                    "add {0}, offset {VAR}",
                    out(reg) base,
                    VAR = sym __PERCPU_CURRENT_EXECUTOR_PTR,
                );
                base as *const usize
            }
            #[cfg(not(target_arch = "x86_64"))]
            {
                #[cfg(target_arch = "aarch64")]
                core::arch::asm!("mrs {}, TPIDR_EL1", out(reg) base);
                #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
                core::arch::asm! ("mv {}, gp", out(reg) base);
                (base + self.offset()) as *const usize
            }
        }
        #[cfg(target_os = "macos")]
        unimplemented!()
    }

    #[inline]
    /// Returns the reference of the per-CPU data on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    pub unsafe fn current_ref_raw(&self) -> &usize {
        &*self.current_ptr()
    }

    #[inline]
    /// Returns the mutable reference of the per-CPU data on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn current_ref_mut_raw(&self) -> &mut usize {
        &mut *(self.current_ptr() as *mut usize)
    }

    /// Manipulate the per-CPU data on the current CPU in the given closure.
    ///
    /// Preemption will be disabled during the call.
    pub fn with_current<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut usize) -> T,
    {
        f(unsafe { self.current_ref_mut_raw() })
    }

    #[inline]
    /// Returns the value of the per-CPU data on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    pub unsafe fn read_current_raw(&self) -> usize {
        #[cfg(not(target_os = "macos"))]
        {
            #[cfg(target_arch = "riscv64")]
            {
                let value: usize;
                core::arch::asm!(
                    "lui {0}, %hi({VAR})",
                    "add {0}, {0}, gp",
                    "ld {0}, %lo({VAR})({0})",
                    out(reg) value,
                    VAR = sym __PERCPU_CURRENT_EXECUTOR_PTR,
                );
                value
            }
            #[cfg(target_arch = "x86_64")]
            {
                let value: usize;
                core::arch::asm!(
                    "mov {0:r}, qword ptr gs:[offset {VAR}]",
                    out(reg) value,
                    VAR = sym __PERCPU_CURRENT_EXECUTOR_PTR
                );
                value
            }
            #[cfg(not(any(target_arch = "riscv64", target_arch = "x86_64")))]
            {
                *self.current_ptr()
            }
        }
        #[cfg(target_os = "macos")]
        unimplemented!()
    }

    #[inline]
    /// Set the value of the per-CPU data on the current CPU.
    ///
    /// # Safety
    ///
    /// Caller must ensure that preemption is disabled on the current CPU.
    pub unsafe fn write_current_raw(&self, val: usize) {
        #[cfg(not(target_os = "macos"))]
        {
            #[cfg(target_arch = "riscv64")]
            {
                core::arch::asm!(
                    "lui {0}, %hi({VAR})",
                    "add {0}, {0}, gp",
                    "sd {1}, %lo({VAR})({0})",
                    out(reg) _, in(reg) val,
                    VAR = sym __PERCPU_CURRENT_EXECUTOR_PTR
                );
            }
            #[cfg(target_arch = "x86_64")]
            {
                core::arch::asm!(
                    "mov qword ptr gs:[offset {VAR}], {0:r}",
                    in(reg) val,
                    VAR = sym __PERCPU_CURRENT_EXECUTOR_PTR
                )
            }
            #[cfg(not(any(target_arch = "riscv64", target_arch = "x86_64")))]
            {
                *(self.current_ptr() as *mut usize) = val
            }
        }
        #[cfg(target_os = "macos")]
        unimplemented!()
    }

    /// Returns the value of the per-CPU data on the current CPU. Preemption will
    /// be disabled during the call.
    pub fn read_current(&self) -> usize {
        unsafe { self.read_current_raw() }
    }

    /// Set the value of the per-CPU data on the current CPU.
    /// Preemption will be disabled during the call.
    pub fn write_current(&self, val: usize) {
        unsafe { self.write_current_raw(val) }
    }
}

/// Gets the pointer to the current task with preemption-safety.
///
/// Preemption may be enabled when calling this function. This function will
/// guarantee the correctness even the current task is preempted.
#[inline]
pub fn current_executor_ptr<T>() -> *const T {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        // on x86, only one instruction is needed to read the per-CPU task pointer from `gs:[off]`.
        CURRENT_EXECUTOR_PTR.read_current_raw() as _
    }
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    unsafe {
        // on RISC-V, reading `CURRENT_EXECUTOR_PTR` requires multiple instruction, so we disable local IRQs.
        let flags = local_irq_save_and_disable();
        let ans = CURRENT_EXECUTOR_PTR.read_current_raw();
        local_irq_restore(flags);
        ans as _
    }
    #[cfg(target_arch = "aarch64")]
    {
        // on ARM64, we use `SP_EL0` to store the task pointer.
        use tock_registers::interfaces::Readable;
        aarch64_cpu::registers::SP_EL0.get() as _
    }
}
/// Sets the pointer to the current task with preemption-safety.
///
/// Preemption may be enabled when calling this function. This function will
/// guarantee the correctness even the current task is preempted.
///
/// # Safety
///
/// The given `ptr` must be pointed to a valid task structure.
#[inline]
pub unsafe fn set_current_executor_ptr<T>(ptr: *const T) {
    #[cfg(target_arch = "x86_64")]
    {
        CURRENT_EXECUTOR_PTR.write_current_raw(ptr as usize)
    }
    #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
    {
        let flags = local_irq_save_and_disable();
        CURRENT_EXECUTOR_PTR.write_current_raw(ptr as usize);
        local_irq_restore(flags)
    }
    #[cfg(target_arch = "aarch64")]
    {
        use tock_registers::interfaces::Writeable;
        aarch64_cpu::registers::SP_EL0.set(ptr as u64)
    }
}

/// A wrapper of [`TaskRef`] as the current task.
pub struct CurrentExecutor(ManuallyDrop<ExecutorRef>);

impl CurrentExecutor {
    pub(crate) fn try_get() -> Option<Self> {
        let ptr: *const Executor = current_executor_ptr();
        if !ptr.is_null() {
            Some(Self(unsafe {
                ManuallyDrop::new(ExecutorRef::from_raw(ptr))
            }))
        } else {
            None
        }
    }

    pub fn get() -> Self {
        Self::try_get().expect("current executor is uninitialized")
    }

    #[allow(unused)]
    /// Converts [`CurrentTask`] to [`TaskRef`].
    pub fn as_executor_ref(&self) -> &ExecutorRef {
        &self.0
    }

    #[allow(unused)]
    pub fn clone(&self) -> ExecutorRef {
        self.0.deref().clone()
    }

    #[allow(unused)]
    pub fn ptr_eq(&self, other: &ExecutorRef) -> bool {
        Arc::ptr_eq(&self.0, other)
    }

    pub unsafe fn init_current(executor: ExecutorRef) {
        let ptr = Arc::into_raw(executor);
        set_current_executor_ptr(ptr);
    }

    pub fn clean_current() {
        let curr = Self::get();
        let Self(arc) = curr;
        ManuallyDrop::into_inner(arc); // `call Arc::drop()` to decrease prev task reference count.
        unsafe { set_current_executor_ptr(0 as *const Executor) };
    }

    pub fn clean_current_without_drop() {
        unsafe { set_current_executor_ptr(0 as *const Executor) };
    }
}

impl Deref for CurrentExecutor {
    type Target = Executor;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
