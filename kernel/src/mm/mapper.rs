use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::{CpuPageMapping, EntryFlags};

/// Trait for memory mapper: maps physical addresses to a virtual addresses.
pub trait MemoryMapper {
    /// Gets the active paging mapping, without locking.
    unsafe fn get_unlocked() -> Self;

    /// Gets a new paging mapping.
    fn get_new() -> Result<CpuPageMapping, MemoryError>;

    /// Translate a virtual address to a physical address (if mapped).
    fn translate(&self, addr: VirtAddr) -> Option<PhysAddr>;

    /// Gets a single physical page and maps it to a given virtual address.
    fn get_and_map_single(&mut self, vaddr: VirtAddr, flags: EntryFlags) -> MemoryResult;

    /// Unmaps a single page and frees the corresponding physical frame.
    fn free_and_unmap_single(&mut self, vaddr: VirtAddr);

    /// Maps a single page.
    fn map_single(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MemoryResult;

    /// Unmaps a single page.
    fn unmap_single(&mut self, vaddr: VirtAddr);

    /// Maps a range of pages to a range of physical frames.
    fn map_range_physical(
        &mut self,
        vaddr: VirtAddr,
        paddr: PhysAddr,
        size: usize,
        flags: EntryFlags,
    ) -> MemoryResult;

    /// Maps a range.
    fn map_range(&mut self, vaddr: VirtAddr, size: usize, flags: EntryFlags) -> MemoryResult;

    /// Unmaps a range.
    fn unmap_range(&mut self, vaddr: VirtAddr, size: usize);

    /// Unmaps a range and frees the corresponding physical frames.
    fn free_and_unmap_range(&mut self, vaddr: VirtAddr, size: usize);

    /// Changes the flags in a range.
    fn change_flags_range(
        &mut self,
        vaddr: VirtAddr,
        size: usize,
        flags: EntryFlags,
    ) -> MemoryResult;
}

/// Memory request result.
pub type MemoryResult = Result<(), MemoryError>;

/// Error during memory request.
#[derive(Debug)]
pub enum MemoryError {
    /// Out of physical memory.
    OOM,
    /// Out of virtual memory (no more virtual memory areas).
    NoMoreVMA,
    /// Invalid memory range (for example partial mapping a Vma out of bounds).
    InvalidRange,
}
