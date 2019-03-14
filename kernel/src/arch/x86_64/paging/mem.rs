use multiboot2::MemoryMapTag;

use crate::arch::x86_64::paging::PAGE_SIZE;
use crate::mem::*;

use super::{ActiveMapping, EntryFlags};
use super::{PhysAddr, VirtAddr};

impl FrameAllocator {
    /// Applies the memory map.
    pub fn apply_mmap(&mut self, tag: &MemoryMapTag) {
        let mut mapping = unsafe { ActiveMapping::get() };

        // Will be the last entry of the PML2 (PML2 exists)
        let tmp_2m_map_addr = VirtAddr::new(511 * 0x200000);
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

            // TODO
            let p2 = mapping.ensure_2m_tables_exist(tmp_2m_map_addr).unwrap();

            unsafe {
                prev_entry_addr.write_volatile(current);
                ActiveMapping::map_2m_with_table(
                    p2,
                    tmp_2m_map_addr,
                    PhysAddr::new(current & !0x1fffff),
                    map_flags,
                );
            }

            prev_entry_addr = tmp_2m_map_addr.as_usize() as *mut _;

            while current < end {
                unsafe { prev_entry_addr.write_volatile(current); }

                // When we reach a new 2 MiB part, map that to our temporary mapping.
                if (current & 0x1fffff) == 0 {
                    unsafe {
                        ActiveMapping::map_2m_with_table(
                            p2,
                            tmp_2m_map_addr,
                            PhysAddr::new(current & !0x1fffff),
                            map_flags,
                        );
                    }
                }

                prev_entry_addr = (tmp_2m_map_addr.as_usize() + (current & 0x1fffff)) as *mut _;
                current += 0x1000;
            }
        }

        // End
        unsafe {
            prev_entry_addr.write_volatile(0);
        }
        self.top = PhysAddr::new(top);

        // TODO: unmap

        self.debug_print_frames();
    }

    /// Debug print all frames.
    fn debug_print_frames(&mut self) {
        println!("debug print frames");

        let mut mapping = unsafe { ActiveMapping::get() };

        while !self.top.is_null() {
            self.consume_and_move_top(|top| {
                print!("{:x} ", top.as_usize() / PAGE_SIZE);
                let vaddr = VirtAddr::new(0x1000);
                mapping.map_single(vaddr, top, EntryFlags::PRESENT).unwrap();
                vaddr
            }).unwrap();
        }

        println!();
    }
}
