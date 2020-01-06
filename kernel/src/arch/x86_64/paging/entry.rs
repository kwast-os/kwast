use bitflags::bitflags;

use super::invalidate;
use super::super::address::{PhysAddr, VirtAddr};

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

/// Bit mask for used entry count.
const USED_COUNT_MASK: u64 = 0x1ff0_0000_0000_0000;

/// Page table entry.
#[repr(transparent)]
pub struct Entry(u64);

/// Entry modifier helper.
pub struct EntryModifier<'a> {
    entry: &'a mut Entry,
    addr: u64,
}

impl<'a> EntryModifier<'a> {
    /// Creates a new entry modifier.
    pub fn new(entry: &'a mut Entry, addr: VirtAddr) -> Self {
        Self {
            entry,
            addr: addr.as_u64(),
        }
    }

    /// Sets the entry.
    pub fn set(&mut self, addr: PhysAddr, flags: EntryFlags) {
        let was_present = self.entry.flags().contains(EntryFlags::PRESENT);

        // W^X policy
        debug_assert_ne!(flags.contains(EntryFlags::WRITABLE), !flags.contains(EntryFlags::NX));

        self.entry.set(addr, flags);

        // See Intel Volume 3: "4.10.4.3 Optional Invalidation" (and footnote)
        if was_present {
            invalidate(self.addr);
        }
    }
}

#[allow(dead_code)]
impl Entry {
    /// Gets the used count part of this entry.
    /// We keep the used count in the first entry available bits.
    pub fn used_count(&self) -> u64 {
        (self.0 >> 52) & 511
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
