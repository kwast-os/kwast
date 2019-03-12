use multiboot2::MemoryMapTag;

use crate::mem::*;

use super::{ActiveMapping, EntryFlags};
use super::{PhysAddr, VirtAddr};

impl FrameAllocator {
    /// Applies the memory map.
    pub fn apply_mmap(&mut self, tag: &MemoryMapTag) {
        let mut mapping = unsafe { ActiveMapping::get() };

        // Will be the last entry of the PML2 (PML2 exists)
        let tmp_2m_map_addr = VirtAddr::new(511 * 0x200000);
        // PML1 exists for the corresponding PML2
        let tmp_4k_map_addr = VirtAddr::new(0x1000);
        // Mapping flags
        let map_flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;

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

            // Sets the first available address ( = top of the stack).
            if unlikely!(top == 0) {
                top = current;
            }

            let p1 = mapping.ensure_4k_tables_exist(tmp_4k_map_addr).unwrap();

            // Process 4K parts at beginning until we have 2M parts.
            while current < end && (current & 0x1fffff) != 0 {
                unsafe {
                    prev_entry_addr.write_volatile(current);
                }
                unsafe {
                    ActiveMapping::map_4k_with_table(p1, tmp_4k_map_addr, PhysAddr::new(current), map_flags);
                }

                prev_entry_addr = tmp_4k_map_addr.as_usize() as *mut _;
                current += 0x1000;
            }

            let p2 = mapping.ensure_2m_tables_exist(tmp_2m_map_addr).unwrap();

            // Process 2 MiB parts until a 2 MiB part doesn't fit anymore.
            // We do this because we only need one invalidation for the whole 2 MiB area.
            while current + 0x200_000 < end {
                unsafe {
                    prev_entry_addr.write_volatile(current);
                }
                unsafe {
                    ActiveMapping::map_2m_with_table(p2, tmp_2m_map_addr, PhysAddr::new(current), map_flags);
                }

                let mut i = 0;
                while i < 0x200_000 {
                    unsafe { prev_entry_addr.write_volatile(current); }

                    prev_entry_addr = (tmp_2m_map_addr.as_usize() + i) as *mut _;
                    i += 0x1000;
                    current += 0x1000;
                }
            }

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
