.set MB_MAGIC,     0xE85250D6   // Multiboot magic
.set MB_ARCH,      0            // i386 protected mode
.set TAG_REQUIRED, 0            // Required tag
.set TAG_OPTIONAL, 1            // Optional tag

// Multiboot header
.section .mboot
.align 8
mboot_hdr_start:
.long MB_MAGIC
.long MB_ARCH
.long mboot_hdr_end - mboot_hdr_start
.long -(MB_MAGIC + MB_ARCH + (mboot_hdr_end - mboot_hdr_start))

// Information request tag
.align 8
info_req_start:
.word 1                           // Type
.word TAG_REQUIRED                // Flags
.long InfoReqEnd - info_req_start // Size of this tag
.long 1                           // Request: command line
.long 6                           // Request: memory map
.long 15                          // Request: ACPI
InfoReqEnd:

// Framebuffer tag
//.align 8
//lfb_start:
//.word 5                         // Type
//.word TAG_REQUIRED              // Flags
//.long lfb_end - lfb_start       // Size of this tag
//.long 1024                      // Width (can be overriden in grub.cfg)
//.long 768                       // Height (can be overriden in grub.cfg)
//.long 32                        // Depth
//lfb_end:

// End tag
.align 8
end_tag_start:
.word 0                           // Type
.word 0                           // Flags
.long end_tag_end - end_tag_start // Size of this tag
end_tag_end:
mboot_hdr_end:

// Preallocate for paging
.section .bss, "aw", @nobits
.align 0x1000
boot_pml4:
.skip 0x1000
boot_pml3:
.skip 0x1000
boot_pml2:
.skip 0x1000
boot_pml1_1:
.skip 0x1000
boot_pml1_2:
.skip 0x1000

.section .text
.code32

.global start
.type start, @function
start:
    // Warning: keep the value of ebx, because that's the register that points to the multiboot struct.

    // Note: I don't bother with checking if Long Mode is supported.
    //       The cpu will just reset when it tries to go to Long Mode if it's not supported.

    // Check magic
    cmp $0x36d76289, %eax
    jne halt

    cld
    mov $stack_top, %esp

    // Map kernel code & data PMLs
    movl $(boot_pml3 + 0x3), boot_pml4 + 0 * 8
    movl $(2 << (52 - 32)), boot_pml4 + 0 * 8 + 4 // Used entry count
    movl $(boot_pml2 + 0x3), boot_pml3 + 0 * 8
    movl $(1 << (52 - 32)), boot_pml3 + 0 * 8 + 4 // Used entry count
    movl $(boot_pml1_1 + 0x3), boot_pml2 + 0 * 8
    movl $(boot_pml1_2 + 0x3), boot_pml2 + 1 * 8
    movl $(2 << (52 - 32)), boot_pml2 + 0 * 8 + 4 // Used entry count
    movl $(511 << (52 - 32)), boot_pml1_1 + 0 * 8 + 4 // Used entry count
    movl $(512 << (52 - 32)), boot_pml1_2 + 0 * 8 + 4 // Used entry count

    // Recursive map
    movl $(boot_pml4 + 0x3), boot_pml4 + 511 * 8
    movl $(1 << (63 - 32)), boot_pml4 + 511 * 8 + 4 // NX-bit

    // Identity map the first 4MiB (except page 0)
    mov $0x1003, %esi
    mov $(boot_pml1_1 + 8 * 1), %edi // Continues to boot_pml1_2
    mov $(511 + 512), %ecx
1:
    mov %esi, (%edi)
    add $0x1000, %esi
    add $8, %edi
    loop 1b

    /**
     * Setup PAT
     * Keep the lower half the same as the startup defaults, but modify the higher half
     * PATs in order (lower):  WB, WT, UC-, UC (same as defaults)
     *               (higher): WC, WP, *reserved*, *reserved*
     */
    mov $(0x06 << 0 | 0x04 << 8 | 0x07 << 16 | 0x00 << 24), %eax
    mov $(0x01 << 0 | 0x05 << 8 | 0x00 << 16 | 0x00 << 24), %edx
    mov $0x0277, %ecx
    wrmsr

    // Enable: PSE, PAE
    mov %cr4, %eax
    orl $(1 << 4 | 1 << 5), %eax
    mov %eax, %cr4

    // Enable: long mode and NX bit
    mov $0xC0000080, %ecx
    rdmsr
    orl $(1 << 8 | 1 << 11), %eax
    wrmsr

    // Enable paging
    mov $boot_pml4, %eax
    mov %eax, %cr3
    mov %cr0, %eax
    orl $(1 << 31), %eax
    mov %eax, %cr0

    // Switch to long mode
    lgdt gdt_descriptor
    jmp $0x8, $1f

.code64
1:
    // The upper 32 bits are undefined when switching from 32-bit to 64-bit or vice versa.
    // Clear the top bits of the stack to prevent issues.
    // ebx contains our multiboot ptr, also clear ebx upper bits.
    mov %esp, %esp
    mov %ebx, %ebx

    // Switch segments
    xor %ax, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    .extern entry
    mov %rbx, %rdi
    call entry

halt:
    hlt
    jmp halt

.section .rodata
gdt:
.quad 0
.quad (1 << 43) | (1 << 44) | (1 << 47) | (1 << 53) // Kernel code segment, only one segment really needed
gdt_descriptor:
.word gdt_descriptor - gdt - 1
.quad gdt

.section .bss, "aw", @nobits
.skip 32768*4
stack_top:
