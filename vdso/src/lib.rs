//! 这里用于内核初始化 vDSO

#![cfg_attr(not(test), no_std)]

extern crate alloc;

#[cfg(test)]
mod test;
use axconfig::SMP;

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

extern "C" {
    fn vdso_sdata();
    fn vdso_edata();
    fn vdso_start();
    fn vdso_end();
}

pub fn get_vdso_base_end() -> (u64, u64, u64, u64) {
    (
        vdso_sdata as _,
        vdso_edata as _,
        vdso_start as _,
        vdso_end as _,
    )
}
