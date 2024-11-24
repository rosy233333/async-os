.globl vdso_start, vdso_end
.balign 0x1000
vdso_start:
	.incbin "vdso/target/riscv64gc-unknown-linux-musl/release/libcops.so"
	.balign 0x1000
vdso_end:
