use core::cmp::max;

use crate::mm;

#[macro_use]
pub mod macros;
pub mod vga_text;
pub mod address;
pub mod interrupts;
pub mod paging;
pub mod port;

// For tests
pub mod qemu;
pub mod serial;

extern "C" {
    static KERNEL_END_PTR: usize;
}

/// Initializes arch-specific stuff.
#[no_mangle]
pub extern "C" fn entry(mboot_addr: usize) {
    interrupts::init();

    // TODO: we should check here for the location of the multiboot structure.
    //       Under normal circumstances it is located directly after the kernel, however the spec
    //       doesn't guarantee this. To simplify the rest of the init we should relocate it if needed.
    let kernel_end = unsafe { &KERNEL_END_PTR as *const _ as usize };
    let mboot_struct = unsafe { multiboot2::load(mboot_addr) };
    let mboot_end = mboot_struct.end_address();
    let reserved_end = max(kernel_end, mboot_end);
    println!("kernel end: {:#x} | mboot end: {:#x}", kernel_end, mboot_end);
    mm::pmm::get().init(&mboot_struct, reserved_end);

    // TODO: map sections correctly
    let sections = mboot_struct.elf_sections_tag().expect("no elf sections found");
    for x in sections.sections() {
        println!("{:#x}-{:#x} {:?}", x.start_address(), x.end_address(), x.flags());
    }

    #[cfg(not(feature = "integration-test"))]
        crate::kernel_main();
    #[cfg(feature = "integration-test")]
        {
            crate::tests::test_main();
            unsafe { qemu::qemu_exit(0); }
        }
}

/// Halt instruction. Waits for interrupt.
#[allow(dead_code)]
pub fn halt() {
    unsafe {
        asm!("hlt" :::: "volatile");
    }
}
