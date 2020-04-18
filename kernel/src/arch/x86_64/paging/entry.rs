use bitflags::bitflags;

use crate::arch::x86_64::address::PhysAddr;

bitflags! {
    pub struct EntryFlags: u64 {
        const PRESENT = 1;
        const WRITABLE = 1 << 1;
        const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        /// No execute.
        const NX = 1 << 63;
        // Cache types (see PAT in boot assembly)
        const CACHE_WB = 0;
        const CACHE_WT = 1 << 3;
        const UNCACHED = 1 << 4;
        const UNCACHABLE = (1 << 3) | (1 << 4);
        const CACHE_WC = 1 << 7;
        const CACHE_WP = (1 << 3) | (1 << 7);
    }
}

/// Page table entry.
pub struct Entry(u64);

// See Intel Volume 3: bits 62:52 are ignored
const USED_COUNT_MASK: u64 = 0x3ff0_0000_0000_0000;

#[allow(dead_code)]
impl Entry {
    /// Gets the used count part of this entry.
    /// We keep the used count in the first entry available bits.
    pub fn used_count(&self) -> u64 {
        (self.0 & USED_COUNT_MASK) >> 52
    }

    /// Raw used count part of this entry.
    pub fn used_count_raw(&self) -> u64 {
        self.0 & USED_COUNT_MASK
    }

    /// Sets the used count part of this entry.
    pub fn set_used_count(&mut self, count: u64) {
        debug_assert!(count <= 512);
        self.0 = (self.0 & !USED_COUNT_MASK) | (count << 52);
    }

    /// Sets the raw value.
    pub unsafe fn set_raw(&mut self, value: u64) {
        self.0 = value;
    }

    /// Gets the raw value.
    pub fn get_raw(&self) -> u64 {
        self.0
    }

    /// Returns true if this entry is unused.
    pub fn is_unused(&self) -> bool {
        self.phys_addr_unchecked().is_null()
    }

    /// Clears the entry.
    #[inline]
    pub fn clear(&mut self) {
        self.0 = self.used_count_raw();
    }

    /// Sets the physical address of this entry, keeps flags.
    #[inline]
    pub fn set_phys_addr(&mut self, addr: PhysAddr) {
        self.0 = self.used_count_raw() | self.flags().bits() | addr.as_u64();
    }

    /// Sets the flags, keeps the physical address.
    #[inline]
    pub fn set_flags(&mut self, flags: EntryFlags) {
        self.0 = self.used_count_raw() | flags.bits() | self.phys_addr_unchecked().as_u64();
    }

    /// Sets the entry to the given address and flags.
    #[inline]
    pub fn set(&mut self, addr: PhysAddr, flags: EntryFlags) {
        self.0 = self.used_count_raw() | flags.bits() | addr.as_u64();
    }

    /// Gets the flags of this entry.
    #[inline]
    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    /// Gets the physical address from page entry.
    pub fn phys_addr(&self) -> Option<PhysAddr> {
        if self.flags().contains(EntryFlags::PRESENT) {
            Some(self.phys_addr_unchecked())
        } else {
            None
        }
    }

    /// Gets the physical address from page entry (unchecked).
    pub fn phys_addr_unchecked(&self) -> PhysAddr {
        PhysAddr::new((self.0 & 0x000f_ffff_ffff_f000) as usize)
    }
}
