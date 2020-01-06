use core::cmp::max;

use multiboot2::BootInformation;

use crate::mm::{self, mapper::MemoryMapper};
use crate::mm::pmm::IteratorFrameAllocator;

use self::{address::PhysAddr, paging::ActiveMapping};
use self::paging::EntryFlags;

#[macro_use]
pub mod macros;
pub mod vga_text;
pub mod address;
pub mod interrupts;
pub mod paging;
pub mod port;
pub mod cpuid;

// For tests
pub mod qemu;
pub mod serial;

/// Initializes arch-specific stuff.
#[no_mangle]
pub extern "C" fn entry(mboot_addr: usize) {
    interrupts::init();

    println!("TODO: check for todos (grep)");
    // TODO
    /*
hoe zit het met locks? lockfree pmm? ABA?
reduce mem in makefile?
fix imports & unused warnings*/
    let mboot_struct = unsafe { multiboot2::load(mboot_addr) };
    init_memory(&mboot_struct);

    #[cfg(not(feature = "integration-test"))]
        crate::kernel_main();
    #[cfg(feature = "integration-test")]
        crate::tests::test_main();
}

/// Init memory management.
fn init_memory(mboot_struct: &BootInformation) {
    // TODO: map sections correctly
    let sections = mboot_struct.elf_sections_tag().expect("no elf sections found");
    for x in sections.sections() {
        println!("{:#x}-{:#x} {:?}", x.start_address(), x.end_address(), x.flags());
    }

    let mmap = mboot_struct.memory_map_tag().expect("memory map unavailable");

    let start = {
        extern "C" {
            static KERNEL_END: usize;
        }

        let kernel_end = unsafe { &KERNEL_END as *const _ as usize };
        let max = max(mboot_struct.end_address() - paging::PHYS_OFF, kernel_end - paging::KERN_OFF);
        ((max + paging::PAGE_MASK) & !paging::PAGE_MASK) as u64
    };
    let end = mmap.memory_areas().map(|a| a.end_address()).max().expect("cannot get end") as usize;

    println!("area: {:#x} - {:#x}", start, end);

    // Do physical map and init pmm
    {
        let mut it = IteratorFrameAllocator::new(
            mboot_struct
                .memory_map_tag().expect("memory map unavailable")
                .memory_areas()
                .map(move |r| {
                    let mut s = r.start_address();
                    if s < start {
                        s = start;
                    }
                    s..r.end_address()
                })
                .flat_map(|r| r.step_by(paging::PAGE_SIZE))
                .map(|p| PhysAddr::new(p as usize)),
        );

        let mut mapping = unsafe { ActiveMapping::get(&mut it) };

        // TODO: assert memorysize<maxsize for physical map

        let pmap_flags: EntryFlags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;

        // First GiB already setup by the boot.
        let mut current: usize = 0x40_000_000;

        // TODO: ugly
        if cpuid::get_ext_edx().contains(cpuid::ExtFeaturesEdx::GB_PAGES) {
            while current < end {
                let addr = PhysAddr::new(current);
                mapping.map_1g(addr.to_pmap(), addr, pmap_flags).unwrap();
                current += 0x40_000_000;
            }
        } else {
            while current < end {
                let addr = PhysAddr::new(current);
                mapping.map_2m(addr.to_pmap(), addr, pmap_flags).unwrap();
                current += 0x200_000;
            }
        }

        mm::pmm::get().init(&mut it.unwrap_frames_iter());
    }
}

/// Halt instruction. Waits for interrupt.
#[allow(dead_code)]
pub fn halt() {
    unsafe {
        asm!("hlt" :::: "volatile");
    }
}
