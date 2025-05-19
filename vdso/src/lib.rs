//! 这里用于内核初始化 vDSO

#![cfg_attr(not(test), no_std)]

extern crate alloc;

#[cfg(test)]
mod test;

mod api;
mod id;
pub use api::*;
pub use id::TaskId;

core::arch::global_asm!(
    r#"
.section .data.vdso
.globl vdso_start, vdso_end
.balign 0x1000
.section .text.vdso
vdso_start:
	.incbin "vdso/target/riscv64gc-unknown-linux-musl/release/libcops.so"
	.balign 0x1000
vdso_end:
    "#,
);
