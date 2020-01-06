ENTRY(start)

SECTIONS {
    . = 1M;

    .rodata : {
        KEEP(*(.mboot))
        *(.rodata .rodata.*)
    }

    .text : {
        *(.text .text.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        *(.bss .bss.*)
    }

    KERNEL_END_PTR = .;
}
