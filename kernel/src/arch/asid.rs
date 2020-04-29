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

pub struct AsidManager {
    /// 1 = at least one available in the bitset corresponding to this bit.
    /// 0 = all used
    global_mask: u64,
    /// Generation counter, used in the case of a roll-over.
    generation: AsidGeneration,
    /// The bitsets for free/used (1 = free, 0 = used).
    entries: [u64; 64],
    /// Bitmap which indicate that entry has not been used yet in this generation.
    /// 1 = never used, 0 = used at least once
    fresh: [u64; 64],
}

impl AsidManager {
    /// Creates a new Address Space Identifier Manager.
    pub const fn new() -> Self {
        Self {
            global_mask: 0b01,//core::u64::MAX,
            generation: 1,
            entries: [core::u64::MAX; 64],
            fresh: [core::u64::MAX; 64],
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
            self.global_mask = 0b01;//core::u64::MAX;
            for i in 0..64 {
                self.entries[i] = core::u64::MAX;
                self.fresh[i] = core::u64::MAX;
            }

            self.generation += 1;

            println!("rollover {:?}", old);
        }

        // Try to reuse the old asid.
        // Only possible if it was used in the previous generation and no other domain has used this
        // already.
        let (global_free, free) = if old.generation == self.generation - 1
            && old.generation > 0
            && self.fresh[(old.number >> 6) as usize] & (1u64 << (old.number as u64 & 63)) > 0
        {
            println!("reuse");
            ((old.number >> 6) as u32, old.number as u32 & 63)
        } else {
            // Search in the global mask for an entry with free asids.
            let global_free = self.global_mask.trailing_zeros();
            unsafe {
                core::intrinsics::assume(global_free < 64);
            }

            // Find a free asid and mark it as used.
            let free = self.entries[global_free as usize].trailing_zeros();
            (global_free, free)
        };

        unsafe {
            core::intrinsics::assume(free < 64);
        }

        self.entries[global_free as usize] ^= 1 << free;

        // Need to update global mask if there are no asids left in this entry now.
        if self.entries[global_free as usize] == 0 {
            self.global_mask ^= 1 << global_free;
        }

        let asid = Asid {
            generation: self.generation,
            number: ((global_free << 6) | free) as u16,
        };

        println!("selected {:?}", asid);

        // If it has been used before in this generation (indicated by a zero bit),
        // invalidate this asid on the cpu.
        if self.fresh[global_free as usize] & (1 << free) == 0 {
            invalidate_asid(asid);
        } else {
            // Not been used yet, mark as "used at least once".
            self.fresh[global_free as usize] ^= 1 << free;
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
            self.entries[global_entry] ^= 1 << (which & 63) as u64;
            if self.entries[global_entry] != 0 {
                self.global_mask |= (1 << global_entry) as u64;
            }
        }
    }
}
