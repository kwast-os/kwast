use multiboot2::BootInformation;
use multiboot2::MemoryMapTag;
use spin::Mutex;

use crate::arch::x86_64::address::PhysAddr;
use crate::arch::x86_64::paging;
use crate::arch::x86_64::paging::PAGE_SIZE;

/// The default frame allocator.
#[derive(Debug)]
struct FrameAllocator {
    reserved_end: PhysAddr,
}

impl FrameAllocator {
    /// Initializes the allocator.
    fn init(&mut self, mboot_struct: &BootInformation, reserved_end: PhysAddr) {
        self.reserved_end = reserved_end.align_up();

        self.apply_mmap(
            mboot_struct.memory_map_tag().expect("Memory map is required")
        );
    }

    /// Empty, uninitialized allocator.
    const fn empty() -> Self {
        FrameAllocator {
            reserved_end: PhysAddr::null(),
        }
    }

    /// Applies the memory map.
    fn apply_mmap(&mut self, tag: &MemoryMapTag) {
        for x in tag.memory_areas() {
            // start is inclusive, end is exclusive

            // There is actually no guarantee about the sanitization of the data.
            // While it is rare that the addresses won't be page aligned, there's apparently been
            // cases before where it wasn't page aligned.
            let mut start = PhysAddr::new(x.start_address() as usize).align_up();
            let end = PhysAddr::new(x.end_address() as usize).align_down();
            //println!("Free memory area: {:?} - {:?} | size={:#x}", start, end, x.size());

            // Adjust for reserved area
            if start < self.reserved_end {
                start = self.reserved_end;
                if start > end {
                    continue;
                }
            }

            let start = start.as_usize();
            let end = end.as_usize();

            println!("{:x} {:x}", start, end);
        }
    }
}

/// The default frame allocator instance.
static ALLOCATOR: Mutex<FrameAllocator> = Mutex::new(FrameAllocator::empty());

pub fn init(mboot_struct: &BootInformation, reserved_end: usize) {
    ALLOCATOR.lock().init(mboot_struct, PhysAddr::new(reserved_end));

    println!("{:#?}", ALLOCATOR);
    loop {}

    // TODO: calc overhead between 2 MiB & 4 KiB and dynamically decide?
}
