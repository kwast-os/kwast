export PATH := $(PATH):$(shell realpath ./toolchain/opt/cross/bin)

ARCH ?= x86_64
BUILD ?= debug
KERNEL_CARGOFLAGS ?=
QEMUFLAGS ?=

RUST_OBJECT  = kernel/target/$(ARCH)-kwast/$(BUILD)/libkernel.a
LD_SCRIPT    = kernel/src/arch/$(ARCH)/link.ld
KERNEL       = build/kernel-$(ARCH)
ISO_FILES    = build/iso
ISO_IMAGE    = build/img.iso
ASM_SOURCES  = $(wildcard kernel/src/arch/$(ARCH)/*.s)
ASM_OBJECTS  = $(patsubst kernel/src/arch/$(ARCH)/%.s, build/arch/$(ARCH)/%.o, $(ASM_SOURCES))

LDFLAGS     = -n -T $(LD_SCRIPT) -s --gc-sections
LD          = $(ARCH)-elf-ld
AS          = $(ARCH)-elf-as

QEMUFLAGS  += -m 512 --enable-kvm -cpu max --serial mon:stdio -device isa-debug-exit,iobase=0xf4,iosize=0x04

USER_CARGOFLAGS =
ifeq ($(BUILD), release)
KERNEL_CARGOFLAGS += --release
USER_CARGOFLAGS += --release
endif

.PHONY: all clean run rust check iso initrd dirs

all: $(KERNEL)

clean:
	@rm -r build/

dirs:
	@mkdir -p $(ISO_FILES)/boot/grub

iso: dirs initrd $(KERNEL)
	@cp kernel/src/arch/$(ARCH)/grub.cfg $(ISO_FILES)/boot/grub
	@cp $(KERNEL) $(ISO_FILES)/boot/kernel
	@grub-mkrescue -o $(ISO_IMAGE) $(ISO_FILES) 2> /dev/null || (echo "grub-mkrescue failed, do you have the necessary dependencies?" && exit 1)

initrd: dirs
	@cd userspace; cargo build $(USER_CARGOFLAGS)
	@cd userspace/target/wasm32-wasi/$(BUILD); (for file in *.wasm; do (wasm-strip "$$file" 2> /dev/null || echo "wasm-strip is not installed. This is not a fatal error. Installing wasm-strip will result in smaller binary files."); done); tar -cf ../../../../$(ISO_FILES)/boot/initrd.tar *.wasm

run: iso
	@qemu-system-$(ARCH) -cdrom $(ISO_IMAGE) $(QEMUFLAGS)

rust:
	@cd kernel; RUST_TARGET_PATH=$(shell pwd) cargo build --target $(ARCH)-kwast.json $(KERNEL_CARGOFLAGS)

check:
	@cd kernel; cargo c --target $(ARCH)-kwast.json $(KERNEL_CARGOFLAGS)

$(KERNEL): rust $(RUST_OBJECT) $(ASM_OBJECTS) $(LD_SCRIPT)
	@$(LD) $(LDFLAGS) -o $(KERNEL) $(ASM_OBJECTS) $(RUST_OBJECT)

build/arch/$(ARCH)/%.o: kernel/src/arch/$(ARCH)/%.s
	@mkdir -p build/arch/$(ARCH)
	@$(AS) -o $@ $<
