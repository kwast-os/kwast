ENTRY(start)

SECTIONS {
    . = 1M;

    .rodata BLOCK(4K) : ALIGN(4K) {
        KEEP(*(.mboot))
        *(.rodata .rodata.*)
    }

    .text BLOCK(4K) : ALIGN(4K) {
        *(.text .text.*)
    }

    .data BLOCK(4K) : ALIGN(4K) {
        *(.data .data.*)
    }

    .bss BLOCK(4K) : ALIGN(4K) {
        *(.bss .bss.*)
    }

    KERNEL_END_PTR = .;
}
