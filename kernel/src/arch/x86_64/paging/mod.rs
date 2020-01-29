use bitflags::bitflags;

use crate::mm::pmm;

use super::address::{PhysAddr, VirtAddr};

use self::entry::Entry;
pub use self::entry::EntryFlags;
use self::table::{Level4, Table};
use crate::mm::mapper::{MemoryMapper, MappingResult, MappingError};

mod entry;
mod table;
mod frame;

/// The page size on this arch.
pub const PAGE_SIZE: usize = 0x1000;

bitflags! {
    /// Represents a PF error.
    #[repr(transparent)]
    pub struct PageFaultError: u64 {
        /// If set, the fault was caused by a protection violation.
        /// Otherwise, it was caused by a non-present page
        const PROTECTION_VIOLATION = 1;
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
    p4: &'static mut Table<Level4>,
}

/// Entry modifier helper.
pub struct EntryModifier<'a> {
    entry: &'a mut Entry,
    addr: u64,
}

/// Invalidates page.
#[inline]
fn invalidate(addr: u64) {
    unsafe { asm!("invlpg ($0)" :: "r" (addr) : "memory"); }
}

impl<'a> EntryModifier<'a> {
    /// Sets the entry.
    pub fn set(&mut self, addr: PhysAddr, flags: EntryFlags) {
        let was_present = self.entry.flags().contains(EntryFlags::PRESENT);

        self.entry.set(addr, flags);

        // See Intel Volume 3: "4.10.4.3 Optional Invalidation" (and footnote)
        if was_present {
            invalidate(self.addr);
        }
    }
}

impl MemoryMapper for ActiveMapping {
    fn get() -> Self {
        let p4_ptr = 0xffffffff_fffff000 as *mut _;
        Self {
            p4: unsafe { &mut *p4_ptr }
        }
    }

    fn translate(&self, addr: VirtAddr) -> Option<PhysAddr> {
        let p2 = self.p4.next_table(addr.p4_index())?.next_table(addr.p3_index())?;

        let p2_entry = &p2.entries[addr.p2_index()];
        if !p2_entry.flags().contains(EntryFlags::PRESENT) {
            return None;
        }

        if p2_entry.flags().contains(EntryFlags::HUGE_PAGE) {
            // We know it is present, so we can just wrap it.
            Some(p2_entry.phys_addr_unchecked())
        } else {
            p2.next_table(addr.p2_index())?.entries[addr.p1_index()].phys_addr()
        }
    }

    fn get_and_map_single(&mut self, vaddr: VirtAddr, flags: EntryFlags) -> MappingResult {
        let mut e = self.get_4k_entry(vaddr)?;

        pmm::get().pop_top(move |top| {
            e.set(top, flags);
            vaddr
        })
    }

    #[inline]
    fn free_and_unmap_single(&mut self, vaddr: VirtAddr) {
        self.unmap_single_internal(vaddr, true)
    }

    fn map_single(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult {
        debug_assert!(paddr.is_page_aligned());
        self.get_4k_entry(vaddr)?.set(paddr, flags);
        Ok(())
    }

    #[inline]
    fn unmap_single(&mut self, vaddr: VirtAddr) {
        self.unmap_single_internal(vaddr, false)
    }

    fn map_range_physical(&mut self, mut vaddr: VirtAddr, mut paddr: PhysAddr, size: usize, flags: EntryFlags) -> MappingResult {
        debug_assert!(vaddr.is_page_aligned());
        debug_assert!(paddr.is_page_aligned());

        let start_vaddr = vaddr;

        for offset in (0..size).step_by(PAGE_SIZE) {
            let res = self.map_single(vaddr, paddr, flags);
            if unlikely!(res.is_err()) {
                self.unmap_range(start_vaddr, offset);
                return res;
            }

            vaddr += PAGE_SIZE;
            paddr += PAGE_SIZE;
        }

        Ok(())
    }

    fn map_range(&mut self, mut vaddr: VirtAddr, size: usize, flags: EntryFlags) -> MappingResult {
        debug_assert!(vaddr.is_page_aligned());

        let start_vaddr = vaddr;

        for offset in (0..size).step_by(PAGE_SIZE) {
            let res = self.get_and_map_single(vaddr, flags);
            if unlikely!(res.is_err()) {
                self.unmap_range(start_vaddr, offset);
                return res;
            }

            vaddr += PAGE_SIZE;
        }

        Ok(())
    }

    fn unmap_range(&mut self, mut vaddr: VirtAddr, size: usize) {
        debug_assert!(vaddr.is_page_aligned());

        for _ in (0..size).step_by(PAGE_SIZE) {
            self.free_and_unmap_single(vaddr);
            vaddr += PAGE_SIZE;
        }
    }
}

#[allow(dead_code)]
impl ActiveMapping {
    /// Unmaps single page, if frame is true, also puts the physical frame on the stack. (internal use only).
    fn unmap_single_internal(&mut self, vaddr: VirtAddr, frame: bool) {
        debug_assert!(vaddr.is_page_aligned());

        let p3 = self.p4.next_table_mut(vaddr.p4_index()).expect("p3 not mapped");
        let p2 = p3.next_table_mut(vaddr.p3_index()).expect("p2 not mapped");
        let p1 = p2.next_table_mut(vaddr.p2_index()).expect("p1 not mapped");

        let e = &mut p1.entries[vaddr.p1_index()];
        debug_assert!(e.flags().contains(EntryFlags::PRESENT));

        if frame {
            pmm::get().push_top(vaddr, e.phys_addr_unchecked());
        }

        e.clear();
        invalidate(vaddr.as_u64());

        p1.decrease_used_count();
        if p1.used_count() == 0 {
            let vaddr = VirtAddr::new(p1 as *mut _ as usize);
            self.unmap_single_internal(vaddr, true);
        }
    }

    /// Gets the entry modifier for a 2 MiB page. Sets the page as used.
    pub fn get_2m_entry(&mut self, vaddr: VirtAddr) -> Result<EntryModifier, MappingError> {
        debug_assert!(vaddr.is_page_aligned());

        let p2 = self.p4
            .next_table_may_create(vaddr.p4_index())?
            .next_table_may_create(vaddr.p3_index())?;

        if p2.entries[vaddr.p2_index()].is_unused() {
            p2.increase_used_count();
        }

        Ok(EntryModifier {
            entry: &mut p2.entries[vaddr.p2_index()],
            addr: vaddr.as_u64(),
        })
    }

    /// Maps a 2MiB page.
    pub fn map_2m(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult {
        debug_assert!(paddr.is_2m_aligned());
        self.get_2m_entry(vaddr)?.set(paddr, flags | EntryFlags::HUGE_PAGE);
        Ok(())
    }

    /// Gets the entry modifier for a 4 KiB page. Sets the page as used.
    pub fn get_4k_entry(&mut self, vaddr: VirtAddr) -> Result<EntryModifier, MappingError> {
        debug_assert!(vaddr.is_page_aligned());

        let p1 = self.p4
            .next_table_may_create(vaddr.p4_index())?
            .next_table_may_create(vaddr.p3_index())?
            .next_table_may_create(vaddr.p2_index())?;

        if p1.entries[vaddr.p1_index()].is_unused() {
            p1.increase_used_count();
        }

        Ok(EntryModifier {
            entry: &mut p1.entries[vaddr.p1_index()],
            addr: vaddr.as_u64(),
        })
    }
}
