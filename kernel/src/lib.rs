#![no_std]
#![feature(
    asm,
    abi_x86_interrupt,
    core_intrinsics,
    ptr_internals,
    alloc_error_handler,
    lang_items
)]
#![cfg_attr(feature = "integration-test", allow(unused_imports), allow(dead_code))]
#![allow(clippy::verbose_bit_mask)]

#[macro_use]
extern crate alloc;

use core::panic::PanicInfo;

use crate::arch::address::VirtAddr;
use arch::interrupts;

#[macro_use]
mod macros;
#[macro_use]
mod arch;
mod mm;
mod tasking;
#[cfg(feature = "integration-test")]
mod tests;
mod util;
mod wasm;

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

/// Run.
pub fn kernel_run(reserved_end: VirtAddr) {
    // May only be called once.
    unsafe {
        mm::init(reserved_end);
    }

    #[cfg(not(feature = "integration-test"))]
    kernel_main();
    #[cfg(feature = "integration-test")]
    {
        use crate::arch::qemu;
        kernel_test_main();
        unsafe {
            qemu::qemu_exit(0);
        }
    }
}

/// Kernel main, called after arch init is done.
#[cfg(not(feature = "integration-test"))]
fn kernel_main() {
    wasm::main::test().unwrap();

    // DEBUG
    /*unsafe {
        let mut stack1 = Stack::new(VirtAddr::new(0x400_000));
        let mut stack2 = Stack::new(VirtAddr::new(0x400_000 - 0x1000));

        stack1.prepare(VirtAddr::new(tasking_test_a as usize));
        stack2.prepare(VirtAddr::new(tasking_test_b as usize));

        switch_to(stack1.as_virt_addr()); // TODO: ugly
    }*/
}

fn tasking_test_a() -> ! {
    loop {
        println!("A");
    }
}

fn tasking_test_b() -> ! {
    loop {
        println!("B");
    }
}

/// Kernel test main, called after arch init is done.
#[cfg(feature = "integration-test")]
fn kernel_test_main() {
    tests::test_main();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
