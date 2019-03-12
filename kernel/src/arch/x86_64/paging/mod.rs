use bitflags::bitflags;

use crate::arch::x86_64::address::{PhysAddr, VirtAddr};
use crate::mem::{get_pmm, MappingError, MappingResult};

pub use self::entry::EntryFlags;
use self::table::{Level1, Level2, Level4, Table};

mod entry;
mod table;
mod mem;

/// The page size on this arch.
pub const PAGE_SIZE: usize = 0x1000;

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

// TODO: locking etc (e.g. when creating new tables)?

pub struct ActiveMapping {
    p4: &'static mut Table<Level4>,
}

#[allow(dead_code)]
impl ActiveMapping {
    /// Gets the active paging mapping.
    /// You need to be very careful if you create a new instance of this!
    pub unsafe fn get() -> Self {
        let p4_ptr = 0xffffffff_fffff000 as *mut _;
        Self {
            p4: &mut *p4_ptr
        }
    }

    /// Invalidates a virtual address.
    #[inline]
    fn invalidate(addr: VirtAddr) {
        unsafe { asm!("invlpg ($0)" :: "r" (addr.as_u64()) : "memory"); }
    }

    /// Translate a virtual address to a physical address (if mapped).
    pub fn translate(&self, addr: VirtAddr) -> Option<PhysAddr> {
        let p2 = self.p4
            .next_table(addr.p4_index())
            .and_then(|p3| p3.next_table(addr.p3_index()));

        if p2.is_none() {
            return None;
        }

        let p2 = p2.unwrap();
        let p2_entry = &p2.entries[addr.p2_index()];
        if !p2_entry.flags().contains(EntryFlags::PRESENT) {
            return None;
        }

        if p2_entry.flags().contains(EntryFlags::HUGE_PAGE) {
            // We know it is present, so we can just wrap it.
            Some(p2_entry.phys_addr_unchecked())
        } else {
            p2.next_table(addr.p2_index())
                .and_then(|p1| p1.entries[addr.p1_index()].phys_addr())
        }
    }

    /// Ensures the tables for 2M page mapping on this virtual address exist.
    fn ensure_2m_tables_exist(&mut self, vaddr: VirtAddr) -> Result<&mut Table<Level2>, MappingError> {
        debug_assert_eq!(vaddr.as_usize() & 0x1fffff, 0);

        let p3 = self.p4.next_table_may_create(vaddr.p4_index())?;
        p3.next_table_may_create(vaddr.p3_index())
    }

    /// Maps a 2MiB page with a given P2 table.
    /// Unsafe because we the P2 table has to correspond to the virtual address.
    unsafe fn map_2m_with_table(p2: &mut Table<Level2>, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) {
        debug_assert_eq!(paddr.as_usize() & 0x1fffff, 0);

        let e = &mut p2.entries[vaddr.p2_index()];
        let was_present = e.flags().contains(EntryFlags::PRESENT);
        e.set(paddr, flags | EntryFlags::HUGE_PAGE);

        // See Intel Volume 3: "4.10.4.3 Optional Invalidation" (and footnote)
        if was_present {
            Self::invalidate(vaddr);
        }
    }

    /// Maps a 2MiB page.
    pub fn map_2m(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult {
        let p2 = self.ensure_2m_tables_exist(vaddr)?;
        unsafe { Self::map_2m_with_table(p2, vaddr, paddr, flags); }
        Ok(())
    }

    /// Gets a 4 KiB physical page and maps it to a given virtual address.
    pub fn get_and_map_4k(&mut self, vaddr: VirtAddr, flags: EntryFlags) -> MappingResult {
        let p1 = self.ensure_4k_tables_exist(vaddr)?;

        get_pmm().consume_and_move_top(move |top| {
            unsafe { Self::map_4k_with_table(p1, vaddr, top, flags); }
            vaddr
        })
    }

    /// Ensures the tables for 4KiB page mapping on this virtual address exist.
    fn ensure_4k_tables_exist(&mut self, vaddr: VirtAddr) -> Result<&mut Table<Level1>, MappingError> {
        debug_assert_eq!(vaddr.as_usize() & 0xfff, 0);

        let p3 = self.p4.next_table_may_create(vaddr.p4_index())?;
        let p2 = p3.next_table_may_create(vaddr.p3_index())?;
        p2.next_table_may_create(vaddr.p2_index())
    }

    /// Maps a 4KiB page with a given P1 table.
    /// Unsafe because we the P1 table has to correspond to the virtual address.
    unsafe fn map_4k_with_table(p1: &mut Table<Level1>, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) {
        debug_assert_eq!(paddr.as_usize() & 0xfff, 0);

        let e = &mut p1.entries[vaddr.p1_index()];
        let was_present = e.flags().contains(EntryFlags::PRESENT);
        e.set(paddr, flags);

        // See Intel Volume 3: "4.10.4.3 Optional Invalidation" (and footnote)
        if was_present {
            Self::invalidate(vaddr);
        }
    }

    /// Maps a 4KiB page.
    pub fn map_4k(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult {
        let p1 = self.ensure_4k_tables_exist(vaddr)?;
        unsafe { Self::map_4k_with_table(p1, vaddr, paddr, flags); }
        Ok(())
    }
}
