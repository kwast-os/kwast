#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]
#![feature(ptr_internals)]

use core::cmp::max;
use core::panic::PanicInfo;

use arch::interrupts;

use crate::arch::x86_64::address::{PhysAddr, VirtAddr};
use crate::arch::x86_64::paging::ActiveMapping;
use crate::arch::x86_64::paging::EntryFlags;

#[macro_use]
mod arch;
#[macro_use]
mod macros;
mod pmm;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // TODO: notify other processors/cores
    println!("{:#?}", info);
    interrupts::disable_ints();
    loop {
        arch::halt();
    }
}

extern "C" {
    static KERNEL_END_PTR: usize;
}

#[no_mangle]
pub extern "C" fn entry(mboot_addr: usize) {
    arch::init();

    // TODO: we should check here for the location of the multiboot structure.
    //       Under normal circumstances it is located directly after the kernel, however the spec
    //       doesn't guarantee this. To simplify the rest of the init we should relocate it if needed.
    let kernel_end = unsafe { &KERNEL_END_PTR as *const _ as usize };
    let mboot_struct = unsafe { multiboot2::load(mboot_addr) };
    let mboot_end = mboot_struct.end_address();
    let reserved_end = max(kernel_end, mboot_end);
    println!("kernel end: {:#x} | mboot end: {:#x}", kernel_end, mboot_end);
    //pmm::init(&mboot_struct, reserved_end);

    let mut active = ActiveMapping::new();
    println!("{:#?}", active.translate(VirtAddr::new(0xb8000)));
    println!("{:#?}", active.translate(VirtAddr::new(0xffffffff_fffff000)));
    println!("{:#?}", active.translate(VirtAddr::new(0)));

    let res = active.map_single(
        VirtAddr::new(0),
        PhysAddr::new(0xb8000),
        EntryFlags::PRESENT,
    );
    println!("----");
    println!("{:#?}", res);
    println!("{:04x}", unsafe { *((0xB8000 + 19 * 80 * 2) as *const u16) });
}
