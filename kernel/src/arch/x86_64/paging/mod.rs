use bitflags::bitflags;

use super::address::{PhysAddr, VirtAddr};

use self::entry::Entry;
pub use self::entry::EntryFlags;
use self::table::{Level4, Table};
use crate::arch::asid::Asid;
use crate::mm::mapper::{MemoryError, MemoryMapper, MemoryResult};
use crate::mm::pmm::with_pmm;
use crate::mm::vma_allocator::MappableVma;
use crate::tasking::scheduler::with_current_thread;
use core::fmt::{Debug, Error, Formatter};
use core::intrinsics::unlikely;
use crate::util::mem_funcs::page_clear;

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

impl Debug for CpuPageMapping {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "CpuPageMapping({:#x})", self.0)
    }
}

/// Entry modifier helper.
pub struct EntryModifier<'a> {
    entry: &'a mut Entry,
    addr: u64,
}

/// Invalidates page.
#[inline(always)]
unsafe fn invalidate_page(addr: u64) {
    llvm_asm!("invlpg ($0)" : : "r" (addr) : "memory");
}

/// Invalidates an Address Space Identifier (PCID on x64).µ
#[inline]
pub fn invalidate_asid(asid_number: u64) {
    debug_assert!(asid_number < 4096);
    let ty: u64 = 1;
    let arg: [u64; 2] = [asid_number, 0];
    // Safety: only has a performance impact on wrong use, not a correctness issue.
    unsafe {
        llvm_asm!("invpcid ($1), $0" : : "r" (ty), "r" (&arg) : "memory");
    }
}

/// Switches to another page mapping.
#[inline(always)]
pub unsafe fn cpu_page_mapping_switch_to(mapping: CpuPageMapping) {
    if get_cpu_page_mapping() != mapping {
        llvm_asm!("movq $0, %cr3" : : "r" (mapping) : "memory" : "volatile");
    }
}

/// Gets the value for CR3.
#[inline(always)]
pub fn get_cpu_page_mapping() -> CpuPageMapping {
    let cr3: CpuPageMapping;
    unsafe {
        llvm_asm!("movq %cr3, $0" : "=r" (cr3) : : "memory" : "volatile");
    }
    cr3
}

impl CpuPageMapping {
    /// Cpu page mapping as physical address representation.
    #[inline]
    pub fn as_phys_addr(self) -> PhysAddr {
        PhysAddr::new((self.0 & !((1 << 63) | 0xfff)) as usize)
    }
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
                invalidate_page(self.addr);
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

impl MemoryMapper for ActiveMapping {
    unsafe fn get_unlocked() -> Self {
        let p4_ptr = 0xffffffff_fffff000 as *mut _;
        Self { p4: &mut *p4_ptr }
    }

    fn get_new() -> Result<CpuPageMapping, MemoryError> {
        // We need to make a temporary mapping to set up the initial state of the PML4.
        // We will do it in the current thread's protection domain.
        with_current_thread(|thread| {
            let domain = thread.domain();
            domain.with(|vma, mapping| {
                // Temporarily map the new PML4 in the current address space.
                let mut p4 = vma.create_vma(PAGE_SIZE)?.map(
                    mapping,
                    0,
                    PAGE_SIZE,
                    EntryFlags::PRESENT | EntryFlags::NX | EntryFlags::WRITABLE,
                )?;

                // Determine physical address of the new PML4.
                let cr3 = mapping
                    .translate(p4.address())
                    .expect("mapping should exist");

                // Copy kernel mappings and clear the others.
                unsafe {
                    page_clear(p4.address().as_mut());
                    let p4_ptr = &mut *p4.address().as_mut::<Table<Level4>>();
                    p4_ptr.entries[0].set_raw(mapping.p4.entries[0].get_raw());
                    p4_ptr.entries[0].set_used_count(2);
                    p4_ptr.entries[511].set(
                        cr3,
                        EntryFlags::PRESENT | EntryFlags::NX | EntryFlags::WRITABLE,
                    );
                }

                // Undo temporary mapping
                // Safety: We free this ourselves.
                //         We need to free manually because we should not free the physical frame.
                vma.insert_region(p4.address(), p4.size());
                unsafe {
                    p4.forget_mapping();
                }
                mapping.unmap_single(p4.address());

                Ok(CpuPageMapping(cr3.as_u64()))
            })
        })
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

    fn get_and_map_single(&mut self, vaddr: VirtAddr, flags: EntryFlags) -> MemoryResult {
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
                        invalidate_page(vaddr.as_u64());
                    }
                }

                pmm.push_top(vaddr, e.phys_addr_unchecked())
            });
        }

        e.clear();
        unsafe {
            invalidate_page(vaddr.as_u64());
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

impl CpuPageMapping {
    /// Sentinel value.
    #[inline]
    pub const fn sentinel() -> CpuPageMapping {
        CpuPageMapping(0)
    }

    /// Applies an Asid to a cpu page mapping.
    #[inline]
    pub fn with_asid(self, asid: Asid) -> CpuPageMapping {
        CpuPageMapping(1 << 63 | self.0 | asid.as_u64())
    }
}
