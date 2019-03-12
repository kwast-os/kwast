#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]
#![feature(ptr_internals)]

use core::panic::PanicInfo;

use arch::interrupts;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{CacheType, EntryFlags};
use crate::mem::PhysMemManagerArchSpecific;

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
pub fn kernel_main() {
    mem::get_pmm().map_page(
        VirtAddr::new(0x400_000),
        EntryFlags::PRESENT | EntryFlags::WRITABLE,
        CacheType::WriteBack,
    ).expect("could not map page");

    let ptr = 0x400_000 as *mut i32;
    unsafe {
        ptr.write_volatile(222);
    }

    println!("end");
}
