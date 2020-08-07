use crate::arch;
use crate::arch::address::VirtAddr;
use crate::arch::paging::{get_cpu_page_mapping, ActiveMapping};
use crate::mm::mapper::MemoryMapper;
use crate::tasking::scheduler::{self, with_current_thread};
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
pub fn page_fault(fault_addr: VirtAddr, ip: VirtAddr) {
    let failed = !with_current_thread(|thread| thread.page_fault(fault_addr));

    if unlikely(failed) {
        if fault_addr.as_usize() < arch::USER_START || ip.as_usize() < arch::USER_START {
            // Kernel fault.
            // TODO: show cause (detect stack overflow for example)
            panic!(
                "Pagefault in kernel, faulting address: {:?} -> {:?}, IP: {:?}, PAGEMAP: {:?}",
                fault_addr,
                unsafe { ActiveMapping::get_unlocked().translate(fault_addr) },
                ip,
                get_cpu_page_mapping()
            );
        } else {
            // Kill the thread.
            println!("Pagefault in thread at {:?}", fault_addr);
            scheduler::thread_exit(u32::MAX); // TODO: exit code
        }
    }
}
