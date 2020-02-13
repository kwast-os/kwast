#![no_std]
#![feature(
    asm,
    abi_x86_interrupt,
    core_intrinsics,
    ptr_internals,
    alloc_error_handler,
    lang_items,
    atomic_mut_ptr
)]
#![cfg_attr(feature = "integration-test", allow(unused_imports), allow(dead_code))]
#![allow(clippy::verbose_bit_mask)]

#[macro_use]
extern crate alloc;

use core::panic::PanicInfo;

use arch::interrupts;

use crate::arch::address::VirtAddr;
use crate::tasking::scheduler;
use crate::tasking::scheduler::SwitchReason;
use crate::tasking::thread::Thread;

#[macro_use]
mod macros;
#[macro_use]
mod arch;
mod mm;
mod sync;
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

/// Kernel main, called after initialization is done.
#[cfg(not(feature = "integration-test"))]
fn kernel_main() {
    wasm::main::test().unwrap();

    let test_thread_a = Thread::create(VirtAddr::new(tasking_test_a as usize)).unwrap();
    let test_thread_b = Thread::create(VirtAddr::new(tasking_test_b as usize)).unwrap();

    scheduler::add_and_schedule_thread(test_thread_a);
    scheduler::add_and_schedule_thread(test_thread_b);

    interrupts::enable();
    interrupts::setup_timer();
    loop {
        scheduler::switch_to_next(SwitchReason::RegularSwitch);
        arch::halt();
    }
}

fn tasking_test_a() -> ! {
    let mut i = 0;
    loop {
        print!("A");
        arch::halt();
        i += 1;
        if i > 20 {
            scheduler::switch_to_next(SwitchReason::Exit);
        }
    }
}

fn tasking_test_b() -> ! {
    loop {
        print!("B");
        arch::halt();
    }
}

/// Kernel test main, called after arch init is done.
#[cfg(feature = "integration-test")]
fn kernel_test_main() {
    tests::test_main();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
