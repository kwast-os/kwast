use crate::arch::address::VirtAddr;
use crate::tasking::scheduler::{self, with_core_scheduler, SwitchReason};
use core::intrinsics::unlikely;

mod alloc;
pub mod avl_interval_tree;
pub mod buddy;
pub mod mapper;
pub mod pmm;
pub mod vma_allocator;

/// Inits memory allocator. May only be called once.
pub unsafe fn init(reserved_end: VirtAddr) {
    alloc::init(reserved_end);
}

/// Page fault handler.
pub fn page_fault(fault_addr: VirtAddr) {
    // TODO: when to panic the kernel?

    let failed =
        !with_core_scheduler(|scheduler| scheduler.get_current_thread().page_fault(fault_addr));
    if unlikely(failed) {
        // Failed, kill the thread.
        println!("Pagefault in thread at {:?}", fault_addr);
        scheduler::switch_to_next(SwitchReason::Exit);
    }
}
