use crate::arch::acpi::hpet::{parse_hpet, HpetData, HpetTable};
use crate::arch::acpi::sdt::{SdtFixedMapping, SdtHeader};
use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::ActiveMapping;
use crate::mm::mapper::MemoryMapper;
use core::convert::TryFrom;
use core::mem::size_of;

pub mod hpet;
mod sdt;

#[derive(Debug)]
pub struct ParsedData {
    pub hpet: Option<HpetData>,
}

pub enum RootSdt {
    Rsdt(PhysAddr),
    Xsdt(PhysAddr),
}

#[allow(dead_code)]
#[repr(u8)]
pub enum AddressSpace {
    SystemMemory = 0,
    SystemIO = 1,
}

#[repr(C, packed)]
pub struct AcpiAddress {
    pub address_space: AddressSpace,
    pub reg_bit_width: u8,
    pub reg_bit_offset: u8,
    _reserved: u8,
    pub address: u64,
}

/// Parses the tables using the root sdt.
/// `vaddr` refers to a free place which is big enough to hold the table information temporarily.
pub fn parse_tables(root_sdt: RootSdt, vaddr: VirtAddr) -> ParsedData {
    let (root_sdt, entry_size) = match root_sdt {
        RootSdt::Rsdt(r) => (r, 4),
        RootSdt::Xsdt(r) => (r, 8),
    };

    let mut result = ParsedData { hpet: None };

    // Safety:
    // We are the only running process right now.
    // This is before the scheduler is setup, so it's not even possible to use `thread.domain()`.
    let mut mapping = unsafe { ActiveMapping::get_unlocked() };
    let root_sdt_mapping =
        SdtFixedMapping::from(&mut mapping, root_sdt, vaddr).expect("root sdt should be mappable");
    let root_sdt = root_sdt_mapping.sdt;

    let sdt_map_addr = vaddr + root_sdt_mapping.size;
    let entries = (root_sdt.length as usize - size_of::<SdtHeader>()) / entry_size;
    //println!("{} entries", entries);

    for i in 0..entries {
        let sdt_ptr_addr = root_sdt as *const _ as usize + size_of::<SdtHeader>() + i * entry_size;
        let sdt_addr = match entry_size {
            4 => PhysAddr::new(unsafe { *(sdt_ptr_addr as *const u32) } as usize),
            8 => PhysAddr::new(
                usize::try_from(unsafe { *(sdt_ptr_addr as *const u64) })
                    .expect("sdt pointer does not fit"),
            ),
            _ => unreachable!("invalid entry size"),
        };

        if let Some(sdt_mapping) = SdtFixedMapping::from(&mut mapping, sdt_addr, sdt_map_addr) {
            let sdt = sdt_mapping.sdt;

            if sdt.name == *b"HPET" {
                // Safety: we know it's HPET.
                result.hpet = Some(parse_hpet(unsafe {
                    &*(sdt as *const _ as *const HpetTable)
                }));
            }

            sdt_mapping.unmap(&mut mapping);
        }
    }

    root_sdt_mapping.unmap(&mut mapping);

    result
}
