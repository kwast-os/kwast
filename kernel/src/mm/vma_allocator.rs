//! Allocator used to split a domain into virtual memory areas.

use spin::Mutex;

use crate::arch::address::VirtAddr;
use crate::arch::paging::PAGE_SIZE;
use crate::mm::avl_interval_tree::AVLIntervalTree;
use crate::mm::mapper::MemoryError;

pub struct VMAAllocator {
    tree: AVLIntervalTree,
}

/// Virtual memory area.
#[derive(Debug)]
pub struct Vma {
    start: VirtAddr,
    len: usize,
}

impl Vma {
    /// Creates a new Vma of the requested size.
    pub fn create(len: usize) -> Result<Self, MemoryError> {
        with_vma_allocator(|allocator| {
            let start = allocator.alloc_region(len).ok_or(MemoryError::NoMoreVMA)?;
            Ok(Self { start, len })
        })
    }

    /// Empty Vma.
    pub const fn empty() -> Self {
        Self {
            start: VirtAddr::null(),
            len: 0,
        }
    }

    /// Gets the starting address.
    #[inline]
    pub fn address(&self) -> VirtAddr {
        self.start
    }

    /// Gets the length.
    #[inline]
    pub fn size(&self) -> usize {
        self.len
    }
}

impl Drop for Vma {
    fn drop(&mut self) {
        if likely!(!self.address().is_null()) {
            with_vma_allocator(|allocator| allocator.insert_region(self.start, self.len));
        }
    }
}

impl VMAAllocator {
    /// Creates a new VMA allocator.
    const fn new() -> Self {
        Self {
            tree: AVLIntervalTree::new(),
        }
    }

    /// Inserts a region.
    pub fn insert_region(&mut self, addr: VirtAddr, len: usize) {
        debug_assert!(addr.is_page_aligned());
        debug_assert!(len % PAGE_SIZE == 0);
        self.tree.return_interval(addr.as_usize(), len);
    }

    /// Allocates a region.
    pub fn alloc_region(&mut self, len: usize) -> Option<VirtAddr> {
        debug_assert!(len % PAGE_SIZE == 0);
        self.tree.find_len(len).map(VirtAddr::new)
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
