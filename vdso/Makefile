TARGET = riscv64gc-unknown-linux-musl
MODE = release
OBJCOPY = riscv64-linux-musl-objcopy

build:
	cd cops && cargo build --release
	@$(OBJCOPY) -S target/$(TARGET)/$(MODE)/libcops.so target/$(TARGET)/$(MODE)/libcops.so
