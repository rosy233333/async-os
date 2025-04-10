//! 这里用于内核初始化 vDSO

#![cfg_attr(not(test), no_std)]

extern crate alloc;

#[cfg(test)]
mod test;

mod api;
mod id;
pub use api::*;
use axconfig::SMP;
pub use id::TaskId;

core::arch::global_asm!(
    r#"
.section .data.vdso
.globl vdso_sdata, vdso_edata, vdso_start, vdso_end
.balign 0x1000
vdso_sdata:
    .fill 0x4000 * {SMP}, 1, 0
vdso_edata:
.section .text.vdso
vdso_start:
	.incbin "vdso/target/riscv64gc-unknown-linux-musl/release/libcops.so"
	.balign 0x1000
vdso_end:
    "#,
    SMP = const SMP,
);
