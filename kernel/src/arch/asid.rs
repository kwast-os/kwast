use crate::arch::paging::invalidate_asid;

pub type AsidGeneration = u32;

/// Address Space Identifier
#[derive(Debug, Copy, Clone)]
pub struct Asid {
    generation: AsidGeneration,
    number: u16,
}

impl Asid {
    /// Gets the 64-bit representation of the asid.
    #[inline]
    pub fn as_u64(self) -> u64 {
        self.number as u64
    }

    /// Gets the generation.
    #[inline]
    pub fn generation(self) -> AsidGeneration {
        self.generation
    }

    /// Invalid asid.
    pub const fn invalid() -> Self {
        Self {
            generation: 0,
            number: 0,
        }
    }
}

struct Entry {
    /// The bitsets for free/used (1 = free, 0 = used).
    used_free: u64,
    /// Bitmap which indicate that entry has not been used yet in this generation.
    /// 1 = never used, 0 = used at least once
    used_in_this_generation: u64,
}

pub struct AsidManager {
    /// 1 = at least one available in the bitset corresponding to this bit.
    /// 0 = all used
    global_mask: u64,
    /// Generation counter, used in the case of a roll-over.
    generation: AsidGeneration,
    /// Bitmasks.
    entries: [Entry; 64],
}

impl Entry {
    pub const fn new() -> Self {
        Self {
            used_free: core::u64::MAX,
            used_in_this_generation: core::u64::MAX,
        }
    }
}

impl AsidManager {
    /// Creates a new Address Space Identifier Manager.
    pub const fn new() -> Self {
        Self {
            global_mask: 0b01, //core::u64::MAX,
            generation: 1,
            entries: [Entry::new(); 64],
        }
    }

    /// Check if the asid is still valid.
    #[inline]
    pub fn is_valid(&self, asid: Asid) -> bool {
        asid.generation == self.generation
    }

    /// Allocates a new Asid.
    pub fn alloc(&mut self, old: Asid) -> Asid {
        unsafe {
            core::intrinsics::assume(old.number < 4096);
        }

        // Roll-over if needed.
        if self.global_mask == 0 {
            self.global_mask = 0b01; //core::u64::MAX;
            for i in 0..64 {
                self.entries[i] = Entry::new();
            }

            self.generation += 1;

            println!("rollover {:?}", old);
        }

        // Try to reuse the old asid.
        // Only possible if it was used in the previous generation and no other domain has used this
        // already.
        let (global_free, free) = if old.generation == self.generation - 1
            && old.generation > 0
            && self.entries[(old.number >> 6) as usize].used_in_this_generation
                & (1u64 << (old.number as u64 & 63)) > 0
        {
            println!("reuse");
            ((old.number >> 6) as usize, old.number as u32 & 63)
        } else {
            // Search in the global mask for an entry with free asids.
            let global_free = self.global_mask.trailing_zeros();
            unsafe {
                core::intrinsics::assume(global_free < 64);
            }

            // Find a free asid and mark it as used.
            let free = self.entries[global_free as usize]
                .used_free
                .trailing_zeros();
            (global_free as usize, free)
        };

        unsafe {
            core::intrinsics::assume(free < 64);
        }

        self.entries[global_free].used_free ^= 1 << free;

        // Need to update global mask if there are no asids left in this entry now.
        if self.entries[global_free].used_free == 0 {
            self.global_mask ^= 1 << global_free;
        }

        let asid = Asid {
            generation: self.generation,
            number: ((global_free << 6) | free as usize) as u16,
        };

        // If it has been used before in this generation (indicated by a zero bit),
        // invalidate this asid on the cpu.
        if self.entries[global_free].used_in_this_generation & (1 << free) == 0 {
            invalidate_asid(asid);
        } else {
            // Not been used yet, mark as "used at least once".
            self.entries[global_free].used_in_this_generation ^= 1 << free;
        }

        asid
    }

    /// Frees an old asid.
    pub fn free(&mut self, which: Asid) {
        if which.generation == self.generation {
            let which = which.number;
            unsafe {
                core::intrinsics::assume(which < 4096);
            }

            let global_entry = (which >> 6) as usize;
            self.entries[global_entry].used_free ^= 1 << (which & 63) as u64;
            if self.entries[global_entry].used_free != 0 {
                self.global_mask |= (1 << global_entry) as u64;
            }
        }
    }
}
