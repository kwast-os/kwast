use bitflags::bitflags;

use crate::arch::x86_64::address::PhysAddr;

bitflags! {
    pub struct EntryFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
    }
}

pub enum CacheType {
    WriteBack = 0,
    WriteThrough = 1 << 3,
    Uncached = 1 << 4,
    Uncacheable = (1 << 3) | (1 << 4),
    WriteCombine = 1 << 7,
    WriteProtect = (1 << 3) | (1 << 7),
}

pub struct Entry(u64);

impl Entry {
    /// Gets the flags of this entry.
    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    /// Gets physical address from frame.
    pub fn phys_addr(&self) -> Option<PhysAddr> {
        if self.flags().contains(EntryFlags::PRESENT) {
            Some(PhysAddr::new((self.0 & 0x000fffff_fffff000) as usize))
        } else {
            None
        }
    }
}