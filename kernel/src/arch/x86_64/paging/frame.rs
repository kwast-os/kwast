use crate::mm::pmm::*;

use super::PhysAddr;

impl DefaultFrameAllocator {
    /// Inits the memory manager.
    pub fn init_internal<I>(&mut self, it: &mut I) -> PhysAddr
        where I: Iterator<Item=PhysAddr> {
        // Previous entry address
        let mut top: usize = 0;
        let mut prev_entry_addr = &mut top as *mut usize;

        for x in it {
            unsafe { prev_entry_addr.write_volatile(x.as_usize()); }
            prev_entry_addr = x.to_pmap().as_usize() as *mut _;
        }

        // End
        unsafe { prev_entry_addr.write_volatile(0); }

        PhysAddr::new(top)
    }
}
