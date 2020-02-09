//! Allocator used to split a domain into virtual memory areas.

use crate::arch::address::VirtAddr;
use crate::arch::x86_64::paging::{ActiveMapping, EntryFlags, PAGE_SIZE};
use crate::mm::avl_interval_tree::AVLIntervalTree;
use crate::mm::mapper::{MappingError, MappingResult, MemoryMapper};
use spin::Mutex;

pub struct VMAAllocator {
    tree: AVLIntervalTree,
}

impl VMAAllocator {
    /// Creates a new VMA allocator.
    const fn new() -> Self {
        Self {
            tree: AVLIntervalTree::new(),
        }
    }

    /// Frees a region.
    pub fn free_region(&mut self, addr: VirtAddr, len: usize) {
        debug_assert!(addr.is_page_aligned());
        debug_assert!(len % PAGE_SIZE == 0);
        self.tree.return_interval(addr.as_usize(), len);
    }

    /// Allocates a region.
    pub fn alloc_region(&mut self, len: usize) -> Option<VirtAddr> {
        debug_assert!(len % PAGE_SIZE == 0);
        self.tree.find_len(len).map(VirtAddr::new)
    }

    /// Allocates a region and maps it.
    pub fn alloc_region_and_map(&mut self, len: usize, flags: EntryFlags) -> MappingResult {
        let addr = self.alloc_region(len).ok_or(MappingError::NoMoreVMA)?;
        let mut mapping = ActiveMapping::get();
        if let e @ Err(_) = mapping.map_range(addr, len, flags) {
            self.free_region(addr, len);
            e
        } else {
            Ok(())
        }
    }
}

static VMA_ALLOCATOR: Mutex<VMAAllocator> = Mutex::new(VMAAllocator::new());

/// Execute something using the VMA allocator.
pub fn with_vma_allocator<F, T>(f: F) -> T
where
    F: FnOnce(&mut VMAAllocator) -> T,
{
    f(&mut VMA_ALLOCATOR.lock())
}
