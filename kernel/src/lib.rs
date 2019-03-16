#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]
#![feature(ptr_internals)]
#![cfg_attr(feature = "integration-test", allow(unused_imports))]

use core::panic::PanicInfo;

use arch::interrupts;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mem::MemoryMapper;

#[macro_use]
mod macros;
#[macro_use]
mod arch;
mod mem;

#[panic_handler]
#[cfg(not(feature = "integration-test"))]
fn panic(info: &PanicInfo) -> ! {
    // TODO: notify other processors/cores
    println!("{:#?}", info);
    interrupts::disable_ints();
    loop {
        arch::halt();
    }
}

/// Kernel main, called after arch init is done.
#[cfg(not(feature = "integration-test"))]
pub fn kernel_main() {
    println!("entered kernel_main");
}

#[panic_handler]
#[cfg(feature = "integration-test")]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("{:#?}", info);
    unsafe { arch::x86_64::qemu::qemu_exit(1); }
}

/// Memory test.
#[cfg(feature = "test-mem")]
pub fn kernel_main() {
    let mut mapping = unsafe { ActiveMapping::get() };

    // Note: `va1` and `va2` are in the same P2
    let va1 = VirtAddr::new(0x400_000);
    let va2 = VirtAddr::new(0xdeadb000);
    let va3 = VirtAddr::new(0x600_000);

    mapping.get_and_map_single(va1, EntryFlags::PRESENT | EntryFlags::WRITABLE)
        .expect("could not map page #1");
    mapping.get_and_map_single(va2, EntryFlags::PRESENT | EntryFlags::WRITABLE)
        .expect("could not map page #2");

    mapping.free_and_unmap_single(va2);

    // Should not PF
    let ptr = va1.as_usize() as *mut i32;
    unsafe { ptr.write_volatile(42); }

    let phys = mapping.translate(va1);
    mapping.free_and_unmap_single(va1);

    mapping.get_and_map_single(va3, EntryFlags::PRESENT)
        .expect("could not map page #3");
    assert_eq!(mapping.translate(va3), phys);
    mapping.free_and_unmap_single(va3);

    unsafe { arch::x86_64::qemu::qemu_exit(0); }
}
