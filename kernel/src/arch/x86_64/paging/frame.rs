use multiboot2::MemoryMapTag;

use super::{invalidate_page, ActiveMapping, EntryFlags, PhysAddr, VirtAddr};
use crate::mm::mapper::MemoryMapper;
use crate::mm::pmm::FrameAllocator;

impl FrameAllocator {
    /// Applies the memory map.
    pub fn apply_mmap(&mut self, tag: &MemoryMapTag, reserved_end: PhysAddr) {
        // Will be the last entry of the PML2 (PML2 exists)
        const P2_IDX: usize = 511;
        let tmp_2m_map_addr = VirtAddr::new(P2_IDX * 0x200_000);

        fn current_to_prev_entry_addr(current: usize) -> *mut usize {
            ((P2_IDX * 0x200_000) | (current & 0x1ff_fff)) as *mut _
        }

        // Mapping flags
        let map_flags =
            EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX | EntryFlags::HUGE_PAGE;

        // Safety: we are the only running thread right now, so no locking is required.
        let mut mapping = unsafe { ActiveMapping::get_unlocked() };
        let mut e = mapping.get_2m_entry(tmp_2m_map_addr).unwrap();

        // Previous entry address
        let mut top: usize = 0;
        let mut prev_entry_addr: *mut usize = &mut top as *mut _;

        //let mut count: usize = 0;

        for x in tag.memory_areas() {
            // There is actually no guarantee about the sanitization of the data.
            // While it is rare that the addresses won't be page aligned, there's apparently been
            // cases before where it wasn't page aligned.
            let mut start = PhysAddr::new(x.start_address() as usize).align_up();
            let end = PhysAddr::new(x.end_address() as usize).align_down();

            // Adjust for reserved area
            if start < reserved_end {
                start = reserved_end;
                if start >= end {
                    continue;
                }
            }

            let mut current = start.as_usize();
            let end = end.as_usize();

            // Initial write for this area is a little bit special because we still
            // need to write to the previous mapping. Otherwise the stack wouldn't be linked.
            // Can't fail.
            unsafe {
                prev_entry_addr.write(current);
            }

            e.set(PhysAddr::new(current & !0x1ff_fff), map_flags);
            prev_entry_addr = current_to_prev_entry_addr(current);

            while current < end {
                unsafe {
                    prev_entry_addr.write(current);
                }

                // When we reach a new 2 MiB part, map that to our temporary mapping.
                if current & 0x1ff_fff == 0 {
                    e.set(PhysAddr::new(current & !0x1ff_fff), map_flags);
                }

                prev_entry_addr = current_to_prev_entry_addr(current);
                current += 0x1000;
                //count += 1;
            }
        }

        // End
        unsafe {
            prev_entry_addr.write(0);
        }
        self.top = PhysAddr::new(top);

        // Unmap
        {
            // Somewhat ugly, but better than complicating other code probably (for now)...
            let p2 = mapping
                .p4
                .next_table_mut(0)
                .unwrap()
                .next_table_mut(0)
                .unwrap();

            p2.entries[P2_IDX].clear();
            p2.decrease_used_count();
            unsafe {
                invalidate_page(tmp_2m_map_addr.as_u64());
            }
        }

        // self.debug_print_frames();
    }

    /// Debug print all frames.
    #[allow(dead_code)]
    fn debug_print_frames(&mut self) {
        let mut mapping = unsafe { ActiveMapping::get_unlocked() };

        while !self.top.is_null() {
            self.pop_top(|top| {
                println!("{:x}", top.as_usize());
                let vaddr = VirtAddr::new(0x1000);
                mapping.map_single(vaddr, top, EntryFlags::PRESENT).unwrap();
                vaddr
            })
            .unwrap();
        }

        println!();
    }
}
