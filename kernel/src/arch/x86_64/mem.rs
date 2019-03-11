use multiboot2::MemoryMapTag;

use crate::mem::*;

use super::address::{PhysAddr, VirtAddr};
use super::paging::{ActiveMapping, CacheType, EntryFlags};

impl FrameAllocatorArchSpecific for FrameAllocator {
    fn map_page(&mut self, vaddr: VirtAddr, flags: EntryFlags, cache_type: CacheType) -> MappingResult {
        if unlikely!(self.top.is_null()) {
            return Err(MappingError::OOM);
        }

        // Maps the page to the destination virtual address, then moves the top.
        let mut mapping = unsafe { ActiveMapping::new() };
        mapping.map_4k(vaddr, self.top, flags, cache_type)?;
        self.move_top(vaddr);

        Ok(())
    }

    fn apply_mmap(&mut self, tag: &MemoryMapTag) {
        let mut mapping = unsafe { ActiveMapping::new() };

        // Will be the last entry of the PML2 (PML2 exists)
        let tmp_2m_map_addr = VirtAddr::new(511 * 0x200000);
        // PML1 exists for the corresponding PML2
        let tmp_4k_map_addr = VirtAddr::new(0x1000);

        // Previous entry address
        let mut top: usize = 0;
        let mut prev_entry_addr: *mut usize = &mut top as *mut _;

        let mut fill_list_entry = |paddr: usize, vaddr: VirtAddr, count: u16, debug: bool| {
            let mut paddr = paddr;
            let mut count = count;

            while count != 0 {
                if debug { println!("{:p}", prev_entry_addr); }
                unsafe { prev_entry_addr.write_volatile(paddr); }

                prev_entry_addr = paddr as *mut _;

                count -= 1;
                paddr += 0x1000;
            }
        };

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

            // Sets the first available address ( = top of the stack).
            if unlikely!(top == 0) {
                top = current;
            }

            // TODO: explain how this works & why 2M mapping

            // Process 4K parts at beginning until we have 2M parts.
            while current < end && (current & 0x1fffff) != 0 {
                unsafe {
                    prev_entry_addr.write_volatile(current);
                }

                mapping.map_4k(tmp_4k_map_addr, PhysAddr::new(current), EntryFlags::PRESENT | EntryFlags::WRITABLE, CacheType::WriteBack)
                    .expect("failed to map");

                prev_entry_addr = tmp_4k_map_addr.as_usize() as *mut _;

                // fill_list_entry(current, tmp_4k_map_addr, 1, false);

                current += 0x1000;
            }
            /*
                        // Process 2M parts until a 2M part doesn't fit anymore.
                        while current + 0x200000 < end {
                            mapping.map_2m(tmp_2m_map_addr, PhysAddr::new(current), EntryFlags::PRESENT | EntryFlags::WRITABLE)
                                .expect("failed to map");

                            println!("2M fill");
                            fill_list_entry(current, tmp_2m_map_addr, 0x200, true);

                            current += 0x200000;
                        }
            */
            /*
                        // Process 4K parts at end.
                        while current < end {
                            mapping.map_4k(tmp_4k_map_addr, PhysAddr::new(current), EntryFlags::PRESENT | EntryFlags::WRITABLE)
                                .expect("failed to map");

                            fill_list_entry(current, 1);

                            current += 0x1000;
                        }*/
        }

        // End
        unsafe {
            prev_entry_addr.write_volatile(0);
        }
        self.top = PhysAddr::new(top);

        // TODO: unmap
    }
}

impl PhysMemManagerArchSpecific for PhysMemManager {
    /// Maps a page.
    fn map_page(&self, vaddr: VirtAddr, flags: EntryFlags, cache_type: CacheType) -> MappingResult {
        let mut mapping = unsafe { ActiveMapping::new() };

        // Pre-allocating the required tables.
        // This can be done without locking the PMM the whole time, which prevents a deadlock.
        mapping.ensure_4k_tables_exist(vaddr)?;

        self.allocator.lock().map_page(vaddr, flags, cache_type)
    }
}
