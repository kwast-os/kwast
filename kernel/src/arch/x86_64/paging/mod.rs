use core::ptr::Unique;

use bitflags::bitflags;

use crate::arch::x86_64::address::{PhysAddr, VirtAddr};

pub use self::entry::{CacheType, EntryFlags};
use self::table::{Level4, Table};

mod entry;
mod table;

/// Page size.
pub const PAGE_SIZE: usize = 0x1000;

/// Error during mapping request.
#[derive(Debug)]
pub enum MappingError {
    /// Out of memory.
    OOM
}

/// Map result.
pub type MappingResult = Result<(), MappingError>;

bitflags! {
    /// Represents a PF error.
    #[repr(transparent)]
    pub struct PageFaultError: u64 {
        /// If set, the fault was caused by a protection violation.
        /// Otherwise, it was caused by a non-present page
        const PROTECTION_VIOLATION = 1 << 0;
        /// If set, a write caused the fault, otherwise it was a read.
        const CAUSED_BY_WRITE = 1 << 1;
        /// If set, fault caused in user mode, otherwise in kernel mode.
        const USER_MODE = 1 << 2;
        /// If set, one or more paging entries had reserved bits set to 1.
        const RSVD = 1 << 3;
        /// If set, fault was caused by instruction fetch.
        const CAUSED_BY_INSTRUCTION_FETCH = 1 << 4;
    }
}

pub struct ActiveMapping {
    p4: Unique<Table<Level4>>,
}

impl ActiveMapping {
    /// Creates a new PML4 owner.
    pub fn new() -> Self {
        let p4_ptr = 0xffffffff_fffff000 as *mut _;
        Self {
            p4: unsafe { Unique::new_unchecked(p4_ptr) }
        }
    }

    /// Gets the PML4 table.
    fn p4(&self) -> &Table<Level4> {
        unsafe { self.p4.as_ref() }
    }

    /// Gets the PML4 table.
    fn p4_mut(&mut self) -> &mut Table<Level4> {
        unsafe { self.p4.as_mut() }
    }

    /// Invalidates a virtual address.
    fn invalidate(addr: VirtAddr) {
        unsafe { asm!("invlpg ($0)" :: "r" (addr.as_u64()) : "memory"); }
    }

    /// Translate a virtual address to a physical address (if mapped).
    pub fn translate(&self, addr: VirtAddr) -> Option<PhysAddr> {
        debug_assert_eq!(addr.page_offset(), 0);
        self.p4()
            .next_table(addr.p4_index())
            .and_then(|p3| p3.next_table(addr.p3_index()))
            .and_then(|p2| p2.next_table(addr.p2_index()))
            .and_then(|p1| p1.entries[addr.p1_index()].phys_addr())
    }

    /// Maps a single page with a caching strategy parameter.
    pub fn map_single_cache(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags, cache_type: CacheType) -> MappingResult {
        let mut p4 = self.p4_mut();
        let p3 = p4.next_table_may_create(vaddr.p4_index())?;
        let p2 = p3.next_table_may_create(vaddr.p3_index())?;
        let p1 = p2.next_table_may_create(vaddr.p2_index())?;

        p1.entries[vaddr.p1_index()].reset_to(paddr, flags, cache_type);

        ActiveMapping::invalidate(vaddr);

        Ok(())
    }

    /// Maps a single page.
    #[inline]
    pub fn map_single(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult {
        self.map_single_cache(vaddr, paddr, flags, CacheType::WriteBack)
    }
}
