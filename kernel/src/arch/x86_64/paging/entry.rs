use bitflags::bitflags;

use crate::arch::x86_64::address::PhysAddr;

bitflags! {
    pub struct EntryFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const HUGE_PAGE = 1 << 7;
        /// No execute.
        const NX = 1 << 63;
        // Cache types
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

#[allow(dead_code)]
impl Entry {
    /// Clears the entry.
    #[inline]
    pub fn clear(&mut self) {
        self.0 = 0;
    }

    /// Sets the physical address of this entry, keeps flags.
    #[inline]
    pub fn set_phys_addr(&mut self, addr: PhysAddr) {
        self.0 = self.flags().bits() | addr.as_u64();
    }

    /// Sets the flags, keeps the physical address.
    #[inline]
    pub fn set_flags(&mut self, flags: EntryFlags) {
        self.0 = flags.bits() | self.phys_addr_unchecked().as_u64();
    }

    /// Sets the entry to the given address and flags.
    #[inline]
    pub fn set(&mut self, addr: PhysAddr, flags: EntryFlags) {
        self.0 = flags.bits() | addr.as_u64();
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
        PhysAddr::new((self.0 & 0x000fffff_fffff000) as usize)
    }
}
