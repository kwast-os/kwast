use crate::arch::address::VirtAddr;

mod alloc;
pub mod avl_interval_tree;
pub mod buddy;
pub mod mapper;
pub mod pmm;

/// Inits memory allocator. May only be called once.
pub unsafe fn init(reserved_end: VirtAddr) {
    alloc::init(reserved_end);
}
