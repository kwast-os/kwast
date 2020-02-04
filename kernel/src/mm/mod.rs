use crate::arch::address::VirtAddr;

pub mod pmm;
pub mod mapper;
pub mod buddy;
pub mod process_heap_assigner;
mod alloc;

/// Inits memory allocator. May only be called once.
pub unsafe fn init(reserved_end: VirtAddr) {
    alloc::init(reserved_end);
}
