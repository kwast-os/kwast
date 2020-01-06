#![no_std]
#![feature(asm)]
#![feature(abi_x86_interrupt)]
#![feature(core_intrinsics)]
#![feature(ptr_internals)]
#![feature(alloc_error_handler)]
#![cfg_attr(feature = "integration-test", allow(unused_imports))]

extern crate alloc;

use core::panic::PanicInfo;

use arch::interrupts;
use alloc::boxed::Box;

#[macro_use]
mod macros;
#[macro_use]
mod arch;
mod mm;
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
pub fn kernel_main() {
    println!("entered kernel_main");

    // TEST
    let test = Box::new([1, 2, 3]);
    println!("{:?}", test);
}
