use bitflags::bitflags;

use crate::arch::x86_64::address::PhysAddr;

bitflags! {
    pub struct EntryFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const HUGE_PAGE = 1 << 7;
        /// No execute.
        const NX = 1 << 63;
    }
}

#[allow(dead_code)]
pub enum CacheType {
    WriteBack = 0,
    WriteThrough = 1 << 3,
    Uncached = 1 << 4,
    Uncacheable = (1 << 3) | (1 << 4),
    WriteCombine = 1 << 7,
    WriteProtect = (1 << 3) | (1 << 7),
}

pub struct Entry(u64);

#[allow(dead_code)]
impl Entry {
    /// Clears the entry.
    pub fn clear(&mut self) {
        self.0 = 0
    }

    /// Sets the physical address of this entry, keeps flags.
    pub fn set_phys_addr(&mut self, addr: PhysAddr) {
        self.0 = self.flags().bits() | addr.as_u64();
    }

    /// Sets the flags, keeps the physical address.
    pub fn set_flags(&mut self, flags: EntryFlags, cache_type: CacheType) {
        self.0 = flags.bits() | (cache_type as u64) | self.phys_addr_unchecked().as_u64();
    }

    /// Sets the entry to the given address and flags.
    pub fn set(&mut self, addr: PhysAddr, flags: EntryFlags, cache_type: CacheType) {
        self.0 = flags.bits() | (cache_type as u64) | addr.as_u64();
    }

    /// Gets the flags of this entry.
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
