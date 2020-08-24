use crate::arch::acpi::sdt::SdtHeader;
use crate::arch::acpi::AcpiAddress;
use crate::arch::address::PhysAddr;
use crate::arch::address::VirtAddr;
use crate::arch::paging::ActiveMapping;
use crate::arch::paging::EntryFlags;
use crate::mm::mapper::MemoryMapper;
use core::convert::TryInto;

#[derive(Debug)]
pub struct HpetData {
    address: PhysAddr,
}

#[must_use = "Hpet should be unmapped after use"]
pub struct Hpet {
    address: VirtAddr,
    clock_period: u64,
}

#[repr(C, packed)]
pub struct HpetTable {
    sdt_header: SdtHeader,
    event_timer_block_id: u32,
    address: AcpiAddress,
    hpet_nr: u8,
    min_clock_tick_in_periodic_mode: u16,
    attributes: u8,
}

/// Parses a Hpet table.
pub fn parse_hpet(table: &HpetTable) -> HpetData {
    let address = PhysAddr::new(
        table
            .address
            .address
            .try_into()
            .expect("address should fit"),
    );

    HpetData { address }
}

impl Hpet {
    /// Creates a mapped `Hpet` from `HpetData`.
    ///
    /// # Safety
    ///
    /// This can cause issues if the `HpetData` or virtual address is invalid.
    ///
    pub unsafe fn from(mapping: &mut ActiveMapping, vaddr: VirtAddr, hpet_data: HpetData) -> Self {
        mapping
            .map_single(
                vaddr,
                hpet_data.address,
                EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX | EntryFlags::UNCACHED,
            )
            .expect("hpet mapping should succeed");

        let mut hpet = Self {
            address: vaddr,
            clock_period: 0,
        };

        // Initialize.
        {
            let capability = hpet.read(0x0);
            let clock_period = capability >> 32; // in femptoseconds
            let num_timers = ((capability >> 8) & 31) as usize + 1;
            println!("{} hpet timers", num_timers);
            for i in 0..num_timers {
                // Disable interrupts on this timer.
                let t0_cfg_cap = hpet.read(0x100 + i * 0x20);
                //println!("{:b}", t0_cfg_cap);
                let t0_cfg_cap = t0_cfg_cap & !(1 << 2);
                hpet.write(0x100 + i * 0x20, t0_cfg_cap);
            }
            let enable = hpet.read(0x10) | (1 << 0);
            hpet.write(0x10, enable);
            hpet.clock_period = clock_period;
        }

        hpet
    }

    /// Reads from a 64 bit register at `offset`.
    ///
    /// # Safety
    ///
    /// This could cause exceptions if the offset is invalid.
    ///
    unsafe fn read(&self, offset: usize) -> u64 {
        self.address
            .as_const::<u64>()
            .add(offset / 8)
            .read_volatile()
    }

    /// Writes to a 64 bit register at `offset`.
    ///
    /// # Safety
    ///
    /// This could cause exceptions if the offset is invalid.
    ///
    unsafe fn write(&self, offset: usize, val: u64) {
        self.address
            .as_mut::<u64>()
            .add(offset / 8)
            .write_volatile(val);
    }

    /// Reads the current counter.
    pub fn counter(&self) -> u64 {
        // Safety: correct offset and in mapped memory
        unsafe { self.read(0xf0) }
    }

    /// Convert a counter value to nanoseconds.
    pub fn counter_to_ns(&self, val: u64) -> u64 {
        (val / 1000) * (self.clock_period / 1000)
    }
}
