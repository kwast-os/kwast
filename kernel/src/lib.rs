#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]
#![feature(ptr_internals)]
#![feature(alloc_error_handler)]
#![allow(incomplete_features)]
#![feature(const_generics)]
#![feature(lang_items)]
#![cfg_attr(feature = "integration-test", allow(unused_imports), allow(dead_code))]
#![allow(clippy::verbose_bit_mask)]

extern crate alloc;

use core::panic::PanicInfo;

use arch::interrupts;
use alloc::boxed::Box;
use crate::arch::x86_64::address::VirtAddr;

#[macro_use]
mod macros;
#[macro_use]
mod arch;
mod mm;
mod util;
#[cfg(feature = "integration-test")]
mod tests;

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
pub fn kernel_main(reserved_end: VirtAddr) {
    println!("entered kernel_main");

    // May only be called once.
    unsafe { mm::init(reserved_end); }

    // TEST
    let test = Box::new([1, 2, 3]);
    println!("{:?}", test);
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
