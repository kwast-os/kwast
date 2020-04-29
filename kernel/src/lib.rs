#![no_std]
#![feature(
    asm,
    abi_x86_interrupt,
    core_intrinsics,
    ptr_internals,
    alloc_error_handler,
    lang_items,
    atomic_mut_ptr,
    assoc_int_consts
)]
#![cfg_attr(feature = "integration-test", allow(unused_imports), allow(dead_code))]
#![allow(clippy::verbose_bit_mask)]

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate memoffset;

#[macro_use]
extern crate wasm_call;

use core::panic::PanicInfo;

use arch::interrupts;

use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use crate::tasking::protection_domain::ProtectionDomain;
use crate::tasking::scheduler;
use crate::tasking::thread::Thread;
use crate::util::boot_module::{BootModule, BootModuleProvider};
use crate::util::tar::Tar;
use core::slice;

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
pub fn kernel_run(reserved_end: VirtAddr, _boot_modules: impl BootModuleProvider) {
    // May only be called once.
    unsafe {
        mm::init(reserved_end);
        arch::late_init();
        tasking::scheduler::init();
    }

    #[cfg(not(feature = "integration-test"))]
    kernel_main(_boot_modules);
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
fn handle_module(module: BootModule) -> Option<()> {
    println!("{:?}", module);

    // Safety: module data is correct.
    let tar = unsafe {
        Tar::from_slice(slice::from_raw_parts(
            module.range.start.as_const(),
            module.range.len,
        ))
    }?;

    // For now, just try to run all files in the tar.
    // Might need a manifest or something alike in the future.
    for file in tar {
        wasm::main::run(file.as_slice()).unwrap_or_else(|e| {
            println!("Could not start: {:?}", e);
        });
    }

    Some(())
}

/// Kernel main, called after initialization is done.
#[cfg(not(feature = "integration-test"))]
fn kernel_main(boot_modules: impl BootModuleProvider) {
    // Make sure the boot modules are mapped.
    if let Some(range) = boot_modules.range() {
        // Safety: we are the only running thread right now, so no locking is required.
        let mut mapping = unsafe { ActiveMapping::get_unlocked() };
        mapping
            .map_range_physical(
                range.start,
                PhysAddr::new(range.start.as_usize()),
                range.len,
                EntryFlags::PRESENT,
            )
            .expect("mapping modules");
    }

    interrupts::enable();
    interrupts::setup_timer();

    // Handle boot modules.
    //for module in boot_modules {
    //    handle_module(module).unwrap_or_else(|| {
    //        println!("Failed to handle module {:?}", module);
    //    });
    //}

    scheduler::thread_yield();
    let mut i = 0;
    while i < 130 {
        unsafe {
            let entry = VirtAddr::new(thread_test as usize);
            let t = Thread::create(ProtectionDomain::new().unwrap(), entry, i).unwrap();
            scheduler::add_and_schedule_thread(t);
        }
        i += 1;
    }

    loop {
        arch::halt();
    }
}

extern "C" fn thread_test(arg: u64) {
    println!("hi {}", arg);
    //scheduler::thread_exit(0);
    loop {}
}

/// Kernel test main, called after arch init is done.
#[cfg(feature = "integration-test")]
fn kernel_test_main() {
    tests::test_main();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
