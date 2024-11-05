//! Platform-specific operations.

cfg_if::cfg_if! {
    if #[cfg(all(target_arch = "riscv64", platform_family = "riscv64-qemu-virt"))] {
        mod riscv64_qemu_virt;
        #[allow(unused_imports)]
        pub use riscv64_qemu_virt::*;
    }
}
