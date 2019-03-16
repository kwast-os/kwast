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


    println!("end");
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
    // TODO: make this a real test

    let mut mapping = unsafe { ActiveMapping::get() };

    mapping.get_and_map_single(VirtAddr::new(0x400_000), EntryFlags::PRESENT | EntryFlags::WRITABLE)
        .expect("could not map page");
    mapping.get_and_map_single(VirtAddr::new(0xdeadb000), EntryFlags::PRESENT | EntryFlags::WRITABLE)
        .expect("could not map page");

    mapping.unmap_single(VirtAddr::new(0xdeadb000));

    let ptr = 0x400_000 as *mut i32;
    unsafe { ptr.write_volatile(42); }

    unsafe { arch::x86_64::qemu::qemu_exit(0); }
}
