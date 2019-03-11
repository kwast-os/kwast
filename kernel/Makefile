export PATH := $(PATH):$(shell realpath ../toolchain/opt/cross/bin)

ARCH ?= x86_64
BUILD ?= debug

RUST_OBJECT = target/src/arch/$(ARCH)/target/$(BUILD)/libkernel.a
LD_SCRIPT   = src/arch/$(ARCH)/link.ld
KERNEL      = build/kernel-$(ARCH)
ISO_FILES   = build/iso
ISO_IMAGE   = build/img.iso
ASM_SOURCES = $(wildcard src/arch/$(ARCH)/*.s)
ASM_OBJECTS = $(patsubst src/arch/$(ARCH)/%.s, build/arch/$(ARCH)/%.o, $(ASM_SOURCES))

QEMUFLAGS   = -m 16 --enable-kvm
LDFLAGS     = -n -T $(LD_SCRIPT) -s --gc-sections
LD          = $(ARCH)-elf-ld
AS          = $(ARCH)-elf-as

CARGOFLAGS  =

ifeq ($(BUILD), release)
CARGOFLAGS += --release
endif

.PHONY: all clean run rust iso

all: $(KERNEL)

clean:
	@rm -r build/

iso: $(KERNEL)
	@mkdir -p $(ISO_FILES)/boot/grub
	@cp src/arch/$(ARCH)/grub.cfg $(ISO_FILES)/boot/grub
	@cp $(KERNEL) $(ISO_FILES)/boot/kernel
	@grub-mkrescue -o $(ISO_IMAGE) $(ISO_FILES)
	@rm -r $(ISO_FILES)

run: iso
	@qemu-system-$(ARCH) -cdrom $(ISO_IMAGE) $(QEMUFLAGS)

rust:
	@RUST_TARGET_PATH=$(shell pwd) cargo xbuild --target src/arch/$(ARCH)/target $(CARGOFLAGS)

$(KERNEL): rust $(RUST_OBJECT) $(ASM_OBJECTS) $(LD_SCRIPT)
	@$(LD) $(LDFLAGS) -o $(KERNEL) $(ASM_OBJECTS) $(RUST_OBJECT)

build/arch/$(ARCH)/%.o: src/arch/$(ARCH)/%.s
	@mkdir -p build/arch/$(ARCH)
	@$(AS) -o $@ $<
