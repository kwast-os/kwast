use bitflags::bitflags;

use super::address::{PhysAddr, VirtAddr};

use self::entry::Entry;
pub use self::entry::EntryFlags;
use self::table::{Level4, Table};
use crate::mm::mapper::{MemoryError, MemoryMapper, MemoryResult};
use crate::mm::pmm::with_pmm;
use core::intrinsics::unlikely;

mod entry;
mod frame;
mod table;

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

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct CpuPageMapping(u64);

pub struct MappingGuard {
    old: CpuPageMapping,
}

/// Entry modifier helper.
pub struct EntryModifier<'a> {
    entry: &'a mut Entry,
    addr: u64,
}

/// Invalidates page.
#[inline(always)]
unsafe fn invalidate(addr: u64) {
    asm!("invlpg ($0)" : : "r" (addr) : "memory");
}

/// Switches to another CR3.
#[inline(always)]
pub unsafe fn cpu_page_mapping_switch_to(cr3: CpuPageMapping) {
    if get_cpu_page_mapping() != cr3 {
        asm!("movq $0, %cr3" : : "r" (cr3) : "memory" : "volatile");
    }
}

/// Gets the value for CR3.
#[inline(always)]
pub fn get_cpu_page_mapping() -> CpuPageMapping {
    let cr3: CpuPageMapping;
    unsafe {
        asm!("movq %cr3, $0" : "=r" (cr3) : : "memory" : "volatile");
    }
    cr3
}

impl<'a> EntryModifier<'a> {
    /// Helper.
    #[inline]
    fn with<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Entry),
    {
        let was_present = self.entry.flags().contains(EntryFlags::PRESENT);

        f(self.entry);

        // See Intel Volume 3: "4.10.4.3 Optional Invalidation" (and footnote)
        if was_present {
            unsafe {
                invalidate(self.addr);
            }
        }
    }

    /// Sets the entry.
    pub fn set(&mut self, addr: PhysAddr, flags: EntryFlags) {
        self.with(|entry| entry.set(addr, flags))
    }

    /// Sets the entry flags.
    pub fn set_flags(&mut self, flags: EntryFlags) {
        self.with(|entry| entry.set_flags(flags))
    }
}

impl Drop for MappingGuard {
    fn drop(&mut self) {
        unsafe {
            cpu_page_mapping_switch_to(self.old);
        }
    }
}

impl MemoryMapper for ActiveMapping {
    unsafe fn get_unlocked() -> Self {
        let p4_ptr = 0xffffffff_fffff000 as *mut _;
        Self { p4: &mut *p4_ptr }
    }

    unsafe fn get_new() -> Result<MappingGuard, MemoryError> {
        // TODO
        let mut mapping = Self::get_unlocked();
        let old = get_cpu_page_mapping();

        // Get a new frame & page for PML4
        let vaddr = VirtAddr::new(0x1000); // TODO: not good, shared between all processes
        let cr3 = mapping.get_and_map_single(
            vaddr,
            EntryFlags::PRESENT | EntryFlags::NX | EntryFlags::WRITABLE,
        )?;

        // Copy kernel mappings and clear the others.
        let p4_ptr = &mut *vaddr.as_mut::<Table<Level4>>();
        p4_ptr.entries[0].set_raw(mapping.p4.entries[0].get_raw());
        p4_ptr.entries[0].set_used_count(2);
        p4_ptr.entries[511].set_raw(mapping.p4.entries[511].get_raw());
        p4_ptr.entries[511].set_phys_addr(cr3);
        for entry in &mut mapping.p4.entries[1..511] {
            entry.set_raw(0);
        }

        // Unmap the temporary mapping.
        mapping.unmap_single_internal(vaddr, false);

        cpu_page_mapping_switch_to(CpuPageMapping(cr3.as_u64()));

        Ok(MappingGuard { old })
    }

    fn translate(&self, addr: VirtAddr) -> Option<PhysAddr> {
        let p2 = self
            .p4
            .next_table(addr.p4_index())?
            .next_table(addr.p3_index())?;

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

    fn get_and_map_single(
        &mut self,
        vaddr: VirtAddr,
        flags: EntryFlags,
    ) -> Result<PhysAddr, MemoryError> {
        let mut e = self.get_4k_entry_may_create(vaddr)?;

        Ok(with_pmm(|pmm| {
            pmm.pop_top(move |top| {
                e.set(top, flags);
                vaddr
            })
        })?)
    }

    #[inline]
    fn free_and_unmap_single(&mut self, vaddr: VirtAddr) {
        self.unmap_single_internal(vaddr, true)
    }

    fn map_single(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MemoryResult {
        debug_assert!(paddr.is_page_aligned());
        self.get_4k_entry_may_create(vaddr)?.set(paddr, flags);
        Ok(())
    }

    #[inline]
    fn unmap_single(&mut self, vaddr: VirtAddr) {
        self.unmap_single_internal(vaddr, false)
    }

    fn map_range_physical(
        &mut self,
        vaddr: VirtAddr,
        paddr: PhysAddr,
        size: usize,
        flags: EntryFlags,
    ) -> MemoryResult {
        debug_assert!(vaddr.is_page_aligned());
        debug_assert!(paddr.is_page_aligned());

        for offset in (0..size).step_by(PAGE_SIZE) {
            let res = self.map_single(vaddr + offset, paddr + offset, flags);
            if unlikely(res.is_err()) {
                self.unmap_range(vaddr, offset);
                return res;
            }
        }

        Ok(())
    }

    fn map_range(&mut self, vaddr: VirtAddr, size: usize, flags: EntryFlags) -> MemoryResult {
        debug_assert!(vaddr.is_page_aligned());

        for offset in (0..size).step_by(PAGE_SIZE) {
            let res = self.get_and_map_single(vaddr + offset, flags);
            if unlikely(res.is_err()) {
                self.free_and_unmap_range(vaddr, offset);
                return Err(res.err().unwrap());
            }
        }

        Ok(())
    }

    fn unmap_range(&mut self, vaddr: VirtAddr, size: usize) {
        debug_assert!(vaddr.is_page_aligned());

        for i in (0..size).step_by(PAGE_SIZE) {
            self.unmap_single(vaddr + i);
        }
    }

    fn free_and_unmap_range(&mut self, vaddr: VirtAddr, size: usize) {
        debug_assert!(vaddr.is_page_aligned());

        for i in (0..size).step_by(PAGE_SIZE) {
            self.free_and_unmap_single(vaddr + i);
        }
    }

    fn change_flags_range(
        &mut self,
        vaddr: VirtAddr,
        size: usize,
        flags: EntryFlags,
    ) -> MemoryResult {
        debug_assert!(vaddr.is_page_aligned());

        for i in (0..size).step_by(PAGE_SIZE) {
            self.get_4k_entry_may_create(vaddr + i)?.set_flags(flags);
        }

        Ok(())
    }
}

#[allow(dead_code)]
impl ActiveMapping {
    /// Unmaps single page, if frame is true, also puts the physical frame on the stack. (internal use only).
    fn unmap_single_internal(&mut self, vaddr: VirtAddr, frame: bool) {
        debug_assert!(vaddr.is_page_aligned());

        let p3 = unwrap_or_return!(self.p4.next_table_mut(vaddr.p4_index()));
        let p2 = unwrap_or_return!(p3.next_table_mut(vaddr.p3_index()));
        let p1 = unwrap_or_return!(p2.next_table_mut(vaddr.p2_index()));

        let e = &mut p1.entries[vaddr.p1_index()];
        if !e.flags().contains(EntryFlags::PRESENT) {
            return;
        }

        if frame {
            with_pmm(|pmm| {
                // The pmm wants to write to the page, so if it is read-only, we need to make it writable.
                let flags = e.flags();
                if !flags.contains(EntryFlags::WRITABLE) {
                    e.set_flags(flags | EntryFlags::WRITABLE | EntryFlags::NX);

                    // Sadly, we have to invalidate here too...
                    unsafe {
                        invalidate(vaddr.as_u64());
                    }
                }

                pmm.push_top(vaddr, e.phys_addr_unchecked())
            });
        }

        e.clear();
        unsafe {
            invalidate(vaddr.as_u64());
        }

        p1.decrease_used_count();

        // This will recursively free the page tables: p2 and p3 also if needed.
        if p1.used_count() == 0 {
            let vaddr = VirtAddr::new(p1 as *mut _ as usize);
            self.unmap_single_internal(vaddr, true);
        }
    }

    /// Gets the entry modifier for a 2 MiB page. Sets the page as used.
    pub fn get_2m_entry(&mut self, vaddr: VirtAddr) -> Result<EntryModifier, MemoryError> {
        debug_assert!(vaddr.is_page_aligned());

        let p2 = self
            .p4
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
    pub fn map_2m(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MemoryResult {
        debug_assert!(paddr.is_2m_aligned());
        self.get_2m_entry(vaddr)?
            .set(paddr, flags | EntryFlags::HUGE_PAGE);
        Ok(())
    }

    /// Gets the entry modifier for a 4 KiB page. Sets the page as used.
    pub fn get_4k_entry_may_create(
        &mut self,
        vaddr: VirtAddr,
    ) -> Result<EntryModifier, MemoryError> {
        debug_assert!(vaddr.is_page_aligned());

        let p1 = self
            .p4
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
