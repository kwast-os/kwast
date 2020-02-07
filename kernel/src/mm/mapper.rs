use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::EntryFlags;

/// Trait for memory mapper: maps physical addresses to a virtual addresses.
pub trait MemoryMapper {
    /// Gets the active paging mapping.
    fn get() -> Self;

    /// Translate a virtual address to a physical address (if mapped).
    fn translate(&self, addr: VirtAddr) -> Option<PhysAddr>;

    /// Gets a single physical page and maps it to a given virtual address.
    fn get_and_map_single(&mut self, vaddr: VirtAddr, flags: EntryFlags) -> MappingResult;

    /// Unmaps a single page and frees the corresponding physical frame.
    fn free_and_unmap_single(&mut self, vaddr: VirtAddr);

    /// Maps a single page.
    fn map_single(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult;

    /// Unmaps a single page.
    fn unmap_single(&mut self, vaddr: VirtAddr);

    /// Maps a range of pages to a range of physical frames.
    fn map_range_physical(
        &mut self,
        vaddr: VirtAddr,
        paddr: PhysAddr,
        size: usize,
        flags: EntryFlags,
    ) -> MappingResult;

    /// Maps a range.
    fn map_range(&mut self, vaddr: VirtAddr, size: usize, flags: EntryFlags) -> MappingResult;

    /// Unmaps a range.
    fn unmap_range(&mut self, vaddr: VirtAddr, size: usize);

    /// Unmaps a range and frees the corresponding physical frames.
    fn free_and_unmap_range(&mut self, vaddr: VirtAddr, size: usize);
}

/// Map result.
pub type MappingResult = Result<(), MappingError>;

/// Error during mapping request.
#[derive(Debug)]
pub enum MappingError {
    /// Out of memory.
    OOM,
}
