use multiboot2::BootInformation;
use spin::Mutex;

use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::EntryFlags;

/// Trait for memory mapper: maps a physical address to a virtua address.
pub trait MemoryMapper {
    /// Gets the active paging mapping.
    /// You need to be very careful if you create a new instance of this!
    unsafe fn get() -> Self;

    /// Translate a virtual address to a physical address (if mapped).
    fn translate(&self, addr: VirtAddr) -> Option<PhysAddr>;

    /// Gets a single physical page and maps it to a given virtual address.
    fn get_and_map_single(&mut self, vaddr: VirtAddr, flags: EntryFlags) -> MappingResult;

    /// Maps a single page.
    fn map_single(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult;
}

/// Map result.
pub type MappingResult = Result<(), MappingError>;

/// Error during mapping request.
#[derive(Debug)]
pub enum MappingError {
    /// Out of memory.
    OOM
}

/// The default frame allocator.
///
/// How does this allocator work?
/// Instead of having a fixed area in the memory to keep the stack,
/// we let each free frame contain a pointer to the next free frame on the stack.
/// This limits the amount of virtual memory we need to reserve.
///
/// When we allocate a frame, we map it to the virtual memory and read the pointer.
/// Then we move the head. There is no unnecessary mapping happening here.
///
/// It is likely that, for an allocation, the data will be accessed anyway after the mapping.
/// For a free, it is likely that the data was already accessed.
#[derive(Debug)]
pub struct FrameAllocator {
    pub reserved_end: PhysAddr,
    pub top: PhysAddr,
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
            top: PhysAddr::null(),
        }
    }

    /// Moves the top of the stack.
    pub fn move_top(&mut self, vaddr: VirtAddr) {
        // Read and set the next top address.
        let ptr = vaddr.as_usize() as *mut usize;
        self.top = PhysAddr::new(unsafe { *ptr });
    }

    /// Consumes the top and moves it. This function is used internally for memory management.
    /// It allows the paging component to get the top directly and let it move.
    /// This is faster than going via `map_page`.
    pub fn consume_and_move_top<F>(&mut self, f: F) -> MappingResult
        where F: FnOnce(PhysAddr) -> VirtAddr {
        if unlikely!(self.top.is_null()) {
            return Err(MappingError::OOM);
        }

        self.move_top(f(self.top));

        Ok(())
    }
}

/// The default frame manager instance.
#[repr(transparent)]
pub struct PhysMemManager {
    allocator: Mutex<FrameAllocator>,
}

impl PhysMemManager {
    /// Inits the physical frame allocator.
    pub fn init(&self, mboot_struct: &BootInformation, reserved_end: usize) {
        self.allocator.lock().init(mboot_struct, PhysAddr::new(reserved_end));
    }

    /// Consumes the top and then lets it move. (internal memory management use only)
    /// See docs at impl.
    #[inline]
    pub fn consume_and_move_top<F>(&self, f: F) -> MappingResult
        where F: FnOnce(PhysAddr) -> VirtAddr {
        self.allocator.lock().consume_and_move_top(f)
    }
}

static PMM: PhysMemManager = PhysMemManager {
    allocator: Mutex::new(FrameAllocator::empty())
};

/// Gets the PMM.
pub fn get_pmm() -> &'static PhysMemManager {
    &PMM
}
