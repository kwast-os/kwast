use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::EntryFlags;
use crate::mm::pmm::FrameAllocator;

/// Trait for memory mapper: maps a physical address to a virtual address.
pub trait MemoryMapper<'a> {
    /// Gets the active paging mapping. Unsafe because need to ensure safety for locking (if needed) and must have physical map.
    unsafe fn get(frame_alloc: &'a mut FrameAllocator) -> Self;

    /// Translate a virtual address to a physical address (if mapped).
    fn translate(&self, addr: VirtAddr) -> Option<PhysAddr>;

    /// Gets a single physical page and maps it to a given virtual address.
    fn get_and_map_single(&mut self, vaddr: VirtAddr, flags: EntryFlags) -> MappingResult;

    /// Unmaps a single page and frees the corresponding physical page.
    fn free_and_unmap_single(&mut self, vaddr: VirtAddr);

    /// Maps a single page.
    fn map_single(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult;

    /// Unmaps a single page.
    fn unmap_single(&mut self, vaddr: VirtAddr);
}

/// Map result.
pub type MappingResult = Result<(), MappingError>;

/// Error during mapping request.
#[derive(Debug)]
pub enum MappingError {
    /// Out of memory.
    OOM
}
