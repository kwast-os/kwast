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

use arch::interrupts;

use crate::arch::address::VirtAddr;
use crate::tasking::scheduler::with_scheduler;
use crate::tasking::thread::Thread;
use crate::tasking::thread::ThreadId;

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
    interrupts::disable();
    loop {
        arch::halt();
    }
}

/// Run.
pub fn kernel_run(reserved_end: VirtAddr) {
    // May only be called once.
    unsafe {
        mm::init(reserved_end);
        tasking::scheduler::init();
    }
    interrupts::enable();

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

    let test_thread = Thread::create(VirtAddr::new(tasking_test_a as usize)).unwrap();

    with_scheduler(|scheduler| {
        scheduler.add_thread(ThreadId::new(), test_thread);
    });
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
