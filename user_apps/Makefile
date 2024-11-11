UAPP_TARGET := riscv64gc-unknown-linux-gnu
MODE := release
TARGET_DIR := target/$(UAPP_TARGET)/$(MODE)
TESTCASE_DIR := ../testcases/riscv64_linux_musl

UAPPS := $(filter-out config.toml target Makefile, $(notdir $(wildcard ./*)))

build_uapps: $(UAPPS)
	@$(foreach uapp, $(UAPPS), cd $(uapp) && cargo build --$(MODE);)
	@$(foreach uapp, $(UAPPS), cp $(TARGET_DIR)/$(uapp) $(TESTCASE_DIR);)

