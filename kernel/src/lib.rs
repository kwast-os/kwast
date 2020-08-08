#![no_std]
#![feature(
    llvm_asm,
    abi_x86_interrupt,
    core_intrinsics,
    ptr_internals,
    alloc_error_handler,
    lang_items,
    atomic_mut_ptr,
    const_in_array_repeat_expressions
)]
#![cfg_attr(feature = "integration-test", allow(unused_imports), allow(dead_code))]
#![allow(clippy::verbose_bit_mask)]

extern crate rlibc;

#[macro_use]
extern crate static_assertions;

#[macro_use]
extern crate alloc;

#[macro_use]
extern crate memoffset;

#[macro_use]
extern crate wasm_call;

use core::panic::PanicInfo;
use core::slice;

use arch::interrupts;

use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use crate::tasking::protection_domain::ProtectionDomain;
use crate::tasking::scheduler;
use crate::tasking::scheme_container::schemes;
use crate::tasking::thread::Thread;
use crate::util::boot_module::{BootModule, BootModuleProvider};
use crate::util::tar::Tar;
use alloc::boxed::Box;

#[macro_use]
mod util;
#[macro_use]
mod arch;
mod mm;
mod sync;
mod tasking;
#[cfg(feature = "integration-test")]
mod tests;
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
        // TODO: we should probably have a manifest file which describes what services should be
        //       in the same domain.
        let domain = ProtectionDomain::new().expect("domain");
        //let domain = with_core_scheduler(|s| s.get_current_thread().domain().clone());
        wasm::main::run(file.as_slice(), domain).unwrap_or_else(|e| {
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
    scheduler::thread_yield();

    // TODO: debug code
    unsafe {
        let entry = VirtAddr::new(thread_test as usize);
        let t = Thread::create(ProtectionDomain::new().unwrap(), entry, 1234).unwrap();
        scheduler::add_and_schedule_thread(t);
    };
    // TODO: debug code
    unsafe {
        let entry = VirtAddr::new(thread2_test as usize);
        let t = Thread::create(ProtectionDomain::new().unwrap(), entry, 1234).unwrap();
        scheduler::add_and_schedule_thread(t);
    };

    // Handle boot modules.
    for module in boot_modules {
        handle_module(module).unwrap_or_else(|| {
            println!("Failed to handle module {:?}", module);
        });
    }

    loop {
        arch::halt();
    }
}

extern "C" fn thread2_test(arg: u64) {
    let self_scheme = schemes().read().get(Box::new([])).unwrap();
    let mut i = 0;
    loop {
        self_scheme.test(i);
        i += 1;
    }
    scheduler::thread_exit(0);
}

extern "C" fn thread_test(arg: u64) {
    let self_scheme = schemes().read().get(Box::new([])).unwrap();
    loop {
        self_scheme.open();
    }
    scheduler::thread_exit(0);
}

/// Kernel test main, called after arch init is done.
#[cfg(feature = "integration-test")]
fn kernel_test_main() {
    tests::test_main();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
