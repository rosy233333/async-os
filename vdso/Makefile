ARCH = riscv64
MODE = release
OBJCOPY = riscv64-linux-musl-objcopy

VDSO_CRATE_PATH = ../../vdso_crate_template
VDSO_TARGET_NAME = libvdsoexample.so

build:
	cd $(VDSO_CRATE_PATH) && make all ARCH=$(ARCH) MODE=$(MODE)
	cd -
	@$(OBJCOPY) -S $(VDSO_CRATE_PATH)/$(VDSO_TARGET_NAME) $(VDSO_TARGET_NAME)
