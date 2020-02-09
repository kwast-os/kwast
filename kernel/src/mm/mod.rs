use crate::arch::address::VirtAddr;
use crate::arch::init_vma_regions;

mod alloc;
pub mod avl_interval_tree;
pub mod buddy;
pub mod mapper;
pub mod pmm;
pub mod vma_allocator;

/// Inits memory allocator. May only be called once.
pub unsafe fn init(reserved_end: VirtAddr) {
    let heap_max_end = alloc::init(reserved_end);
    init_vma_regions(heap_max_end);
}
