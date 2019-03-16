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

    let mut mapping = unsafe { ActiveMapping::get() };

    mapping.get_and_map_single(VirtAddr::new(0x400_000), EntryFlags::PRESENT | EntryFlags::WRITABLE)
        .expect("could not map page");
    mapping.get_and_map_single(VirtAddr::new(0xdeadb000), EntryFlags::PRESENT | EntryFlags::WRITABLE)
        .expect("could not map page");

    let ptr = 0x400_000 as *mut i32;
    unsafe {
        ptr.write_volatile(222);
        println!("{}", *ptr);
    }
    let ptr = 0xdeadb000 as *mut i32;
    unsafe {
        println!("{}", *ptr);
    }

    mapping.unmap_single(VirtAddr::new(0xdeadb000));
    // This should crash
    mapping.unmap_single(VirtAddr::new(0xdeada000));

    println!("end");
}

#[cfg(feature = "test-mem")]
pub fn kernel_main() {
    // TODO: real test, use serial output, hide qemu etc
    serial_println!("integration test");

    unsafe { arch::x86_64::qemu::qemu_exit(0); }
}
