export PATH := $(PATH):$(shell realpath ./toolchain/opt/cross/bin)

ARCH ?= x86_64
BUILD ?= debug
CARGOFLAGS ?=
QEMUFLAGS ?=

RUST_OBJECT  = kernel/target/$(ARCH)-kwast/$(BUILD)/libkernel.a
LD_SCRIPT    = kernel/src/arch/$(ARCH)/link.ld
KERNEL       = build/kernel-$(ARCH)
ISO_FILES    = build/iso
ISO_IMAGE    = build/img.iso
ASM_SOURCES  = $(wildcard kernel/src/arch/$(ARCH)/*.s)
ASM_OBJECTS  = $(patsubst kernel/src/arch/$(ARCH)/%.s, build/arch/$(ARCH)/%.o, $(ASM_SOURCES))
INITRD_FILES = hello.wasm 

LDFLAGS     = -n -T $(LD_SCRIPT) -s --gc-sections
LD          = $(ARCH)-elf-ld
AS          = $(ARCH)-elf-as

QEMUFLAGS  += -m 512 --enable-kvm -cpu max --serial mon:stdio

ifeq ($(BUILD), release)
CARGOFLAGS += --release
endif

.PHONY: all clean run rust iso initrd

all: $(KERNEL)

clean:
	@rm -r build/

iso: initrd $(KERNEL)
	@cp kernel/src/arch/$(ARCH)/grub.cfg $(ISO_FILES)/boot/grub
	@cp $(KERNEL) $(ISO_FILES)/boot/kernel
	@grub-mkrescue -o $(ISO_IMAGE) $(ISO_FILES) 2> /dev/null || (echo "grub-mkrescue failed, do you have the necessary dependencies?" && exit 1)

initrd: $(INITRD_FILES)
	@mkdir -p $(ISO_FILES)/boot/grub
	@tar -cf $(ISO_FILES)/boot/initrd.tar -C wasm_samples $(INITRD_FILES)

hello.wasm:

run: iso
	@qemu-system-$(ARCH) -cdrom $(ISO_IMAGE) $(QEMUFLAGS)

rust:
	@cd kernel; RUST_TARGET_PATH=$(shell pwd) cargo xbuild --target $(ARCH)-kwast.json $(CARGOFLAGS)

$(KERNEL): rust $(RUST_OBJECT) $(ASM_OBJECTS) $(LD_SCRIPT)
	@$(LD) $(LDFLAGS) -o $(KERNEL) $(ASM_OBJECTS) $(RUST_OBJECT)

build/arch/$(ARCH)/%.o: kernel/src/arch/$(ARCH)/%.s
	@mkdir -p build/arch/$(ARCH)
	@$(AS) -o $@ $<
