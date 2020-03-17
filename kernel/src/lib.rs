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
use crate::arch::ArchBootModuleProvider;
use crate::tasking::scheduler;
use crate::tasking::scheduler::SwitchReason;
use crate::util::boot_module::BootModule;

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
    interrupts::disable();
    println!("{:#?}", info);
    loop {
        arch::halt();
    }
}

/// Run.
pub fn kernel_run(reserved_end: VirtAddr, boot_modules: ArchBootModuleProvider) {
    // May only be called once.
    unsafe {
        mm::init(reserved_end);
        tasking::scheduler::init();
    }

    #[cfg(not(feature = "integration-test"))]
    kernel_main(boot_modules);
    #[cfg(feature = "integration-test")]
    {
        use crate::arch::qemu;
        kernel_test_main();
        unsafe {
            qemu::qemu_exit(0);
        }
    }
}

/// Handle module.
fn handle_module(module: BootModule) {
    println!("{:?}", module);
}

/// Kernel main, called after initialization is done.
#[cfg(not(feature = "integration-test"))]
fn kernel_main(boot_modules: ArchBootModuleProvider) {
    for module in boot_modules {
        handle_module(module);
    }

    // Start three threads to test.
    wasm::main::test().unwrap();
    wasm::main::test().unwrap();
    wasm::main::test().unwrap();

    interrupts::enable();
    interrupts::setup_timer();
    scheduler::switch_to_next(SwitchReason::RegularSwitch);
    loop {
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
