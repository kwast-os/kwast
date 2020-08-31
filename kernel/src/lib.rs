#![no_std]
#![feature(
    llvm_asm,
    abi_x86_interrupt,
    core_intrinsics,
    ptr_internals,
    alloc_error_handler,
    lang_items,
    atomic_mut_ptr,
    const_in_array_repeat_expressions,
    bool_to_option,
    maybe_uninit_extra,
    maybe_uninit_ref
)]
#![cfg_attr(feature = "integration-test", allow(unused_imports), allow(dead_code))]
#![allow(clippy::verbose_bit_mask)]
#![allow(clippy::new_without_default)]

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
use crate::arch::hpet;
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use crate::mm::tcb_alloc::with_thread;
use crate::tasking::scheduler::{self, thread_exit, with_core_scheduler, with_current_thread};
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
    unsafe {
        // May only be called once.
        mm::init(reserved_end);
    }
    arch::late_init();
    tasking::scheduler::init();

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
    println!("Handle module {:?}", module);

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
        //let domain = ProtectionDomain::new().expect("domain");
        let domain = with_current_thread(|t| t.domain().clone());
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

    {
        let hpet = hpet().unwrap();
        let start = hpet.counter();
        for i in 0..10000000 {
            scheduler::thread_yield();
        }
        let t = hpet.counter() - start;
        println!("{}ns", hpet.counter_to_ns(t) / 10000000);
    }

    // TODO: debug code
    unsafe {
        let entry = VirtAddr::new(thread_test as usize);
        //let domain = ProtectionDomain::new().unwrap();
        let domain = with_current_thread(|t| t.domain().clone());
        let t = Thread::create(domain, entry, 1234).unwrap();
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

extern "C" fn thread_test(_arg: u64) {
    let hpet = hpet().unwrap();
    let self_scheme = schemes().read().open_self(Box::new([])).unwrap();
    let (scheme, _handle) = self_scheme.scheme_and_handle().unwrap();
    //scheme.open(-1).unwrap();
    let a = hpet.counter();
    for i in 0..(10000 - 1) {
        //scheme.open(i).unwrap();
    }
    let b = hpet.counter();
    println!("open: {}ns", hpet.counter_to_ns(b - a) / (10000 - 1));
    println!();
    println!();
    let x = with_core_scheduler(|s| s.current_thread_id());
    let a = hpet.counter();
    for _ in 0..10000 {
        with_thread(x, |t| assert_eq!(t.id, x));
    }
    let b = hpet.counter();
    println!("with_thread: {}ns", hpet.counter_to_ns(b - a) / 10000);
    println!();
    println!();

    thread_exit(123);
}

/// Kernel test main, called after arch init is done.
#[cfg(feature = "integration-test")]
fn kernel_test_main() {
    tests::test_main();
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
