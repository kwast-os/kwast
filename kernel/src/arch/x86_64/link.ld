ENTRY(start)

KERN_OFF = 0xffffffff80000000;

SECTIONS {
    . = 1M;

    .rodata.boot : {
        KEEP(*(.mboot))
        build/arch/x86_64/mboot.o(.rodata)
    }

    .text.boot : {
        build/arch/x86_64/mboot.o(.text)
    }

    .bss.boot (NOLOAD) : {
        build/arch/x86_64/mboot.o(.bss)
    }

    . += KERN_OFF;

    .rodata : AT(ADDR(.rodata) - KERN_OFF) {
        *(EXCLUDE_FILE (*build/arch/x86_64/mboot.o) .rodata .rodata.*)
    }

    .text : AT(ADDR(.text) - KERN_OFF) {
        *(EXCLUDE_FILE (*build/arch/x86_64/mboot.o) .text .text.*)
    }

    .data : AT(ADDR(.data) - KERN_OFF) {
        *(.data .data.*)
    }

    .bss : AT(ADDR(.bss) - KERN_OFF) {
        *(EXCLUDE_FILE (*build/arch/x86_64/mboot.o) .bss .bss.*)
    }

    KERNEL_END = .;

    CPUID_EXT_EDX = __CPUID_EXT_EDX + KERN_OFF;
}
