TARGET = riscv64gc-unknown-linux-musl
MODE = release
OBJCOPY = riscv64-linux-musl-objcopy -g -R .comment -X
OBJDUMP = riscv64-linux-musl-objdump -t -T -r -R -D

PWD := $(shell pwd)
COPS_DIR := $(PWD)/cops
COPS := $(PWD)/target/$(TARGET)/$(MODE)/libcops.so


build:
	@cd $(COPS_DIR) && cargo build --features=no_std --target riscv64gc-unknown-linux-musl --release
	@$(OBJCOPY) $(COPS) $(COPS)
	@$(OBJDUMP) $(COPS) > $(COPS).dump

clean:
	@rm -rf target
