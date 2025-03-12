TARGET = riscv64gc-unknown-linux-musl
MODE = release
OBJCOPY = riscv64-linux-musl-objcopy

build:
	cd cops && cargo build --release
	riscv64-linux-musl-objdump -d target/riscv64gc-unknown-linux-musl/release/libcops.so > ../libcops.txt
	@$(OBJCOPY) -S target/$(TARGET)/$(MODE)/libcops.so target/$(TARGET)/$(MODE)/libcops.so
