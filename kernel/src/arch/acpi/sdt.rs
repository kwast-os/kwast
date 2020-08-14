use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::{ActiveMapping, EntryFlags, PAGE_SIZE};
use crate::mm::mapper::MemoryMapper;
use core::num::Wrapping;
use core::slice;

#[repr(C, packed)]
pub struct SdtHeader {
    pub name: [u8; 4],
    pub length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

#[must_use = "Fixed mapping must be released"]
pub struct SdtFixedMapping<'a> {
    pub sdt: &'a SdtHeader,
    pub size: usize,
}

impl<'a> SdtFixedMapping<'a> {
    /// Maps an sdt.
    pub fn from(mapping: &mut ActiveMapping, paddr: PhysAddr, vaddr: VirtAddr) -> Option<Self> {
        let flags = EntryFlags::PRESENT | EntryFlags::NX;

        // Map two pages, read length, and map more if needed.
        // We need at least two pages because we're not sure if the `length` is aligned
        // inside a single page and the offset inside the header fits in a single page.
        let aligned_down = paddr.align_down();
        let offset = paddr.as_usize() - aligned_down.as_usize();
        mapping
            .map_range_physical(vaddr, aligned_down, 2 * PAGE_SIZE, flags)
            .expect("early sdt mapping should succeed");
        // Safety: we just mapped this.
        let sdt = unsafe { &*(vaddr + offset).as_const::<SdtHeader>() };
        let required_mapped_size =
            (paddr + sdt.length as usize).align_up().as_usize() - aligned_down.as_usize();

        // Safety limit
        if required_mapped_size > 4096 * 1024 {
            return None;
        }

        // Map more if necessary.
        if required_mapped_size > 2 * PAGE_SIZE {
            mapping
                .map_range_physical(
                    vaddr + 2 * PAGE_SIZE,
                    aligned_down + 2 * PAGE_SIZE,
                    required_mapped_size - 2 * PAGE_SIZE,
                    flags,
                )
                .expect("early sdt mapping extend should succeed");
        }

        let result = Self {
            sdt,
            size: required_mapped_size,
        };

        // Validation
        // Safety: fully mapped and exists during the function call.
        if unsafe { Self::validate_sdt(sdt) } {
            Some(result)
        } else {
            result.unmap(mapping);
            None
        }
    }

    /// Unmaps this sdt.
    pub fn unmap(self, mapping: &'a mut ActiveMapping) {
        mapping.unmap_range(
            VirtAddr::new(self.sdt as *const _ as usize).align_down(),
            self.size,
        );
    }

    /// Validates the SDT using its checksum.
    ///
    /// # Safety
    ///
    /// This is only safe if the `SdtHeader` comes from the ACPI tables.
    /// We have no way to verify whether the length is legit.
    ///
    unsafe fn validate_sdt(sdt: &SdtHeader) -> bool {
        let slice = slice::from_raw_parts(sdt as *const _ as *const u8, sdt.length as usize);
        slice.iter().map(|x| Wrapping(*x)).sum::<Wrapping<_>>() == Wrapping(0)
    }
}
