TARGET := riscv64gc-unknown-none-elf
MODE := release
APP_DIR := src/bin
TARGET_DIR := target/$(TARGET)/$(MODE)

# $(wildcard 模式)
# $() 是MakeFile 中的函数调用语法
# wildcard 是文件名模式匹配函数
# 模式 则是要匹配的文件路径模式
APPS := $(wildcard $(APP_DIR)/*.rs)  # 匹配 APP_DIR 目录下的所有 .rs 文件

# $(patsubst 模式,替换为,文本列表) 在文本列表中查找匹配模式的部分，并进行替换
# $(APP_DIR)/%.rs 匹配该路径下的所有 .rs 文件
# $(TARGET_DIR)/% 替换为该路径下的 去掉 .rs 扩展名
# $(APPS) 输入的文件路径列表
ELFS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%, $(APPS))

BINS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(APPS))

OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64

elf: $(APPS)
	@python3 build.py

binary: elf
	@$(foreach elf, $(ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)

build: binary

clean:
	@cargo clean

.PHONY: elf binary build clean