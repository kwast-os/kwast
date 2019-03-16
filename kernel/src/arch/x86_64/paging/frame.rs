use multiboot2::MemoryMapTag;

use crate::mem::*;

use super::{ActiveMapping, EntryFlags, invalidate, PAGE_SIZE, PhysAddr, VirtAddr};

impl FrameAllocator {
    /// Applies the memory map.
    pub fn apply_mmap(&mut self, tag: &MemoryMapTag) {
        // Will be the last entry of the PML2 (PML2 exists)
        const P2_IDX: usize = 511;
        let tmp_2m_map_addr = VirtAddr::new(P2_IDX * 0x200000);
        // Mapping flags
        let map_flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX | EntryFlags::HUGE_PAGE;

        let mut mapping = unsafe { ActiveMapping::get() };
        let mut e = mapping.get_2m_entry(tmp_2m_map_addr).unwrap();

        // Previous entry address
        let mut top: usize = 0;
        let mut prev_entry_addr: *mut usize = &mut top as *mut _;

        for x in tag.memory_areas() {
            // There is actually no guarantee about the sanitization of the data.
            // While it is rare that the addresses won't be page aligned, there's apparently been
            // cases before, where it wasn't page aligned.
            let mut start = PhysAddr::new(x.start_address() as usize).align_up();
            let end = PhysAddr::new(x.end_address() as usize).align_down();

            // Adjust for reserved area
            if start < self.reserved_end {
                start = self.reserved_end;
                if start > end {
                    continue;
                }
            }

            let mut current = start.as_usize();
            let end = end.as_usize();

            // Initial write for this area is a little bit special because we still
            // need to write to the previous mapping. Otherwise the stack wouldn't be linked.
            // Can't fail.
            unsafe { prev_entry_addr.write_volatile(current); }

            e.set(PhysAddr::new(current & !0x1fffff), map_flags);
            prev_entry_addr = tmp_2m_map_addr.as_usize() as *mut _;

            while current < end {
                unsafe { prev_entry_addr.write_volatile(current); }

                // When we reach a new 2 MiB part, map that to our temporary mapping.
                if (current & 0x1fffff) == 0 {
                    e.set(PhysAddr::new(current & !0x1fffff), map_flags);
                }

                prev_entry_addr = (tmp_2m_map_addr.as_usize() + (current & 0x1fffff)) as *mut _;
                current += 0x1000;
            }
        }

        // End
        unsafe { prev_entry_addr.write_volatile(0); }
        self.top = PhysAddr::new(top);

        // Unmap
        {
            // Somewhat ugly, but better than complicating other code probably (for now)...
            let p2 = mapping.p4
                .next_table_mut(0).unwrap()
                .next_table_mut(0).unwrap();

            p2.entries[P2_IDX].clear();
            p2.decrease_used_count();
            invalidate(tmp_2m_map_addr.as_u64());
        }

        // Debug
        //self.debug_print_frames();
    }

    /// Debug print all frames.
    #[allow(dead_code)]
    fn debug_print_frames(&mut self) {
        println!("debug print frames");

        let mut mapping = unsafe { ActiveMapping::get() };

        while !self.top.is_null() {
            self.pop_top(|top| {
                print!("{:x} ", top.as_usize() / PAGE_SIZE);
                let vaddr = VirtAddr::new(0x1000);
                mapping.map_single(vaddr, top, EntryFlags::PRESENT).unwrap();
                vaddr
            }).unwrap();
        }

        println!();
    }
}
