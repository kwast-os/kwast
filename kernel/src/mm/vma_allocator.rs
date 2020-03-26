//! Allocator used to split an address space domain into virtual memory areas.

use crate::arch::address::VirtAddr;
use crate::arch::paging::{ActiveMapping, EntryFlags, PAGE_SIZE};
use crate::mm::avl_interval_tree::AVLIntervalTree;
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::sync::spinlock::Spinlock;
use core::intrinsics::{likely, unlikely};
use core::ptr::write_bytes;

/// Virtual memory allocator.
pub struct VMAAllocator {
    tree: AVLIntervalTree,
}

/// Virtual memory area.
#[derive(Debug, Eq, PartialEq)]
pub struct Vma {
    start: VirtAddr,
    size: usize,
}

pub trait MappableVma {
    /// Gets the starting address.
    fn address(&self) -> VirtAddr;

    /// Gets the size.
    fn size(&self) -> usize;

    /// Checks if the address is contained within the area.
    fn is_contained(&self, addr: VirtAddr) -> bool {
        self.address().as_usize() <= addr.as_usize()
            && (self.address() + self.size()).as_usize() > addr.as_usize()
    }
}

/// Mapped of a Vma (may be partially).
#[derive(Debug, Eq, PartialEq)]
pub struct MappedVma {
    vma: Vma,
}

/// Lazily mapped Vma, mapped on access.
pub struct LazilyMappedVma {
    vma: Vma,
    /// The flags to use when mapping the memory.
    flags: EntryFlags,
    /// The size of the real mapped part.
    allocated_size: usize,
}

impl Vma {
    /// Dummy Vma.
    pub const fn dummy() -> Self {
        Self {
            start: VirtAddr::null(),
            size: 0,
        }
    }

    /// Creates a new Vma of the requested size.
    pub fn create(size: usize) -> Result<Self, MemoryError> {
        with_vma_allocator(|allocator| allocator.alloc_region(size))
            .map(|start| Self { start, size })
            .ok_or(MemoryError::NoMoreVMA)
    }

    /// Convert to mapped Vma.
    pub fn map(
        self,
        map_off: usize,
        map_size: usize,
        flags: EntryFlags,
    ) -> Result<MappedVma, MemoryError> {
        debug_assert!(map_off % PAGE_SIZE == 0);
        debug_assert!(map_size % PAGE_SIZE == 0);

        if unlikely(map_off >= self.size || map_off + map_size > self.size) {
            Err(MemoryError::InvalidRange)
        } else {
            let mut mapping = ActiveMapping::get();
            mapping.map_range(self.start + map_off, map_size, flags)?;

            Ok(MappedVma { vma: self })
        }
    }

    /// Convert to a lazily mapped Vma.
    pub fn map_lazily(self, allocated_size: usize, flags: EntryFlags) -> LazilyMappedVma {
        // TODO: move me? & validate
        ActiveMapping::get().map_range(self.start, allocated_size, flags).unwrap();

        LazilyMappedVma {
            vma: self,
            flags,
            allocated_size,
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
        self.size
    }
}

impl MappedVma {
    /// Dummy Vma.
    pub const fn dummy() -> Self {
        Self { vma: Vma::dummy() }
    }

    /// Is dummy?
    pub fn is_dummy(&self) -> bool {
        *self == Self::dummy()
    }
}

impl MappableVma for MappedVma {
    #[inline]
    fn address(&self) -> VirtAddr {
        self.vma.address()
    }

    #[inline]
    fn size(&self) -> usize {
        self.vma.size()
    }
}

impl LazilyMappedVma {
    /// Dummy mapped Vma.
    pub const fn dummy() -> Self {
        Self {
            vma: Vma::dummy(),
            flags: EntryFlags::empty(),
            allocated_size: 0,
        }
    }

    /// Expands the allocated size.
    /// Returns the old size on success, an error on failure.
    pub fn expand(&mut self, amount: usize) -> Result<usize, MemoryError> {
        let old_size = self.allocated_size;
        let new_size = old_size + amount;

        if new_size > self.vma.size {
            Err(MemoryError::InvalidRange)
        } else {
            self.allocated_size = new_size;
            Ok(old_size)
        }
    }

    /// Try handle a page fault.
    pub fn try_handle_page_fault(&mut self, fault_addr: VirtAddr) -> bool {
        if likely(self.is_contained(fault_addr)) {
            let mut mapping = ActiveMapping::get();
            let flags = self.flags();
            let map_addr = fault_addr.align_down();

            // After the mapping is successful, we need to clear the memory to avoid information leaks.
            if mapping.get_and_map_single(map_addr, flags).is_ok() {
                let ptr: *mut u8 = map_addr.as_mut();
                // Safe because valid pointer and valid size.
                unsafe {
                    write_bytes(ptr, 0, PAGE_SIZE);
                }

                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Gets the flags to use when mapping the memory.
    #[inline]
    pub fn flags(&self) -> EntryFlags {
        self.flags
    }
}

impl MappableVma for LazilyMappedVma {
    #[inline]
    fn address(&self) -> VirtAddr {
        self.vma.address()
    }

    #[inline]
    fn size(&self) -> usize {
        self.allocated_size
    }
}

impl Drop for Vma {
    fn drop(&mut self) {
        if likely(!self.address().is_null()) {
            with_vma_allocator(|allocator| allocator.insert_region(self.start, self.size));
        }
    }
}

fn drop_mapping(start: VirtAddr, size: usize) {
    let mut mapping = ActiveMapping::get();
    // We don't need to tell the exact mapped range, we own all of this.
    // For an empty mapping, the size will be zero, so we don't have to check that.
    mapping.free_and_unmap_range(start, size);
}

impl Drop for MappedVma {
    fn drop(&mut self) {
        drop_mapping(self.address(), self.size());
    }
}

impl Drop for LazilyMappedVma {
    fn drop(&mut self) {
        drop_mapping(self.address(), self.size());
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

static VMA_ALLOCATOR: Spinlock<VMAAllocator> = Spinlock::new(VMAAllocator::new());

/// Execute something using the VMA allocator.
pub fn with_vma_allocator<F, T>(f: F) -> T
where
    F: FnOnce(&mut VMAAllocator) -> T,
{
    f(&mut VMA_ALLOCATOR.lock())
}
