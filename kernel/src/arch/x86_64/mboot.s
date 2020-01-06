.set MB_MAGIC,     0xE85250D6           // Multiboot magic
.set MB_ARCH,      0                    // i386 protected mode
.set TAG_REQUIRED, 0                    // Required tag
.set TAG_OPTIONAL, 1                    // Optional tag
.set PHYS_OFF,     0xffff800000000000   // Offset to physical pages
.set KERN_OFF,     0xffffffff80000000   // Offset to kernel pages

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

.text
.code32

.global start
.type start, @function
start:
    // Warning: ebx holds the address to the multiboot struct.
    //          cpuid will overwrite it, but we need it in rdi anyway...
    mov %ebx, %edi

    // Note: I don't bother with checking if Long Mode is supported.
    //       The cpu will just reset when it tries to go to Long Mode if it's not supported.

    // Check magic
    cmp $0x36d76289, %eax
    jne halt

    cld

    // Get extended CPU features
    mov $0x80000001, %eax
    cpuid
    #mov %ecx, __CPUID_EXT_ECX
    mov %edx, __CPUID_EXT_EDX

    // We want to do the initial mapping of first 1 GiB direct physical map.
    movl $(BOOT_PML3_KERN + 0x3), BOOT_PML4 + 0 * 8
    movl $(BOOT_PML3_PHYS + 0x3), BOOT_PML4 + 256 * 8
    movl $(BOOT_PML3_KERN + 0x3), BOOT_PML4 + 511 * 8
    // Entry count should be 2, because we will unmap the first mapping later.
    movl $(2 << (52 - 32)), BOOT_PML4 + 0 * 8 + 4

    // Check if 1 GiB pages are supported, if it is, map the direct physical map using GiB pages.
    // Otherwise use 2 MiB pages.
    btl $26, %edx
    jnc .no_gib_pages

    // Setup GiB mapping starting from address 0.
    // Set used entry count to 1 and NX bit.
    movl $0x083, BOOT_PML3_PHYS + 0 * 8
    movl $(1 << (52 - 32) | 1 << (63 - 32)), BOOT_PML3_PHYS + 0 * 8 + 4

    jmp .map_kernel

.no_gib_pages:
    // Setup mapping using 512 2 MiB pages.
    // Set used entry count to 512 and NX bit.
    movl $(BOOT_PML2_PHYS + 0x3), BOOT_PML3_PHYS + 0 * 8
    movl $(512 << (52 - 32) | 1 << (63 - 32)), BOOT_PML3_PHYS + 0 * 8 + 4

    mov $0x083, %eax
    mov $(BOOT_PML2_PHYS + 0 * 8), %ebx
    mov $512, %ecx
1:
    mov %eax, (%ebx)
    add $0x200000, %eax
    add $8, %ebx
    loop 1b

.map_kernel:
    // TODO: should we map to -1 GiB or maybe even less mapping for kernel?

    // Map first 2 MiB for kernel (except 0)
    movl $(BOOT_PML2_KERN + 0x3), BOOT_PML3_KERN + 0 * 8
    movl $(BOOT_PML2_KERN + 0x3), BOOT_PML3_KERN + 510 * 8
    // Note: We will unmap entry 0 later, set used count to 1
    movl $(1 << (52 - 32)), BOOT_PML3_KERN + 0 * 8 + 4
    movl $(BOOT_PML1 + 0x3), BOOT_PML2_KERN + 0 * 8
    // Set entry count to 511
    movl $(511 << (52 - 32)), BOOT_PML2_KERN + 0 * 8 + 4

    mov $0x1003, %eax
    mov $(BOOT_PML1 + 1 * 8), %ebx
    mov $511, %ecx
1:
    mov %eax, (%ebx)
    add $0x1000, %eax
    add $8, %ebx
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

    // Enable paging, WP
    mov $BOOT_PML4, %eax
    mov %eax, %cr3
    mov %cr0, %eax
    orl $(1 << 31 | 1 << 16), %eax
    mov %eax, %cr0

    // Switch to long mode
    lgdt gdt_descriptor
    jmp $0x8, $1f

.code64
1:
    // The upper 32 bits are undefined when switching from 32-bit to 64-bit or vice versa.
    // A 32-bit move will clear out the top 32-bits.
    mov %edi, %edi
    mov $PHYS_OFF, %rsi
    add %rsi, %rdi

    mov $(stack_top + KERN_OFF), %rsp

    // Switch segments
    movw $0, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    // No need to reload segments
    lgdt virt_gdt_descriptor

    jmp 1f + KERN_OFF
1:
    // Unmap lower addresses
    movl $0, BOOT_PML3_KERN + 0 * 8
    movl $0, (BOOT_PML4 + KERN_OFF) + 0 * 8

    .extern entry
    movq $entry, %rax
    call *%rax

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
virt_gdt_descriptor:
.word gdt_descriptor - gdt - 1
.quad gdt + KERN_OFF

.bss

// Preallocate for paging
.align 0x1000
.globl BOOT_PML4
BOOT_PML4:
.skip 0x1000
BOOT_PML3_KERN:
.skip 0x1000
BOOT_PML3_PHYS:
.skip 0x1000
BOOT_PML2_KERN:
.skip 0x1000
BOOT_PML2_PHYS:
.skip 0x1000
BOOT_PML1:
.skip 0x1000

// Stack
.skip 16384
stack_top:

// CPUID stuff
.align 4
#.globl CPUID_EXT_ECX
#__CPUID_EXT_ECX:
#.skip 4
.globl __CPUID_EXT_EDX
__CPUID_EXT_EDX:
.skip 4
