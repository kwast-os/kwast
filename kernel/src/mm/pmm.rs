use multiboot2::BootInformation;
use spin::Mutex;

use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::EntryFlags;

/// Trait for memory mapper: maps a physical address to a virtual address.
pub trait MemoryMapper {
    /// Gets the active paging mapping.
    /// You need to be very careful if you create a new instance of this!
    unsafe fn get() -> Self;

    /// Translate a virtual address to a physical address (if mapped).
    fn translate(&self, addr: VirtAddr) -> Option<PhysAddr>;

    /// Gets a single physical page and maps it to a given virtual address.
    fn get_and_map_single(&mut self, vaddr: VirtAddr, flags: EntryFlags) -> MappingResult;

    /// Unmaps a single page and frees the corresponding physical page.
    fn free_and_unmap_single(&mut self, vaddr: VirtAddr);

    /// Maps a single page.
    fn map_single(&mut self, vaddr: VirtAddr, paddr: PhysAddr, flags: EntryFlags) -> MappingResult;

    /// Unmaps a single page.
    fn unmap_single(&mut self, vaddr: VirtAddr);
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
/// There is no additional mapping compared to the classical stack approach:
/// * When a page is being allocated it'll need to be mapped anyway.
/// * When a page is being freed it was already mapped.
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

    /// Pops the top and moves it. This function is used internally for memory management.
    /// It allows the paging component to get the top directly and let it move.
    /// This is faster than going via `map_page`.
    pub fn pop_top<F>(&mut self, f: F) -> MappingResult
        where F: FnOnce(PhysAddr) -> VirtAddr {
        if unlikely!(self.top.is_null()) {
            return Err(MappingError::OOM);
        }

        // Read and set the next top address.
        let ptr = f(self.top).as_usize() as *const usize;
        self.top = PhysAddr::new(unsafe { *ptr });

        Ok(())
    }

    /// Similar to `pop_top`.
    /// This pushes a new top on the stack and links it to the previous top.
    pub fn push_top(&mut self, vaddr: VirtAddr, paddr: PhysAddr) {
        let ptr = vaddr.as_usize() as *mut usize;
        unsafe { ptr.write_volatile(self.top.as_usize()); }
        self.top = paddr;
    }
}

/// The default frame manager instance.
#[repr(transparent)]
pub struct PhysMemManager {
    allocator: Mutex<FrameAllocator>,
}

static PMM: PhysMemManager = PhysMemManager {
    allocator: Mutex::new(FrameAllocator::empty())
};

impl PhysMemManager {
    /// Inits the physical frame allocator.
    pub fn init(&self, mboot_struct: &BootInformation, reserved_end: usize) {
        self.allocator.lock().init(mboot_struct, PhysAddr::new(reserved_end));
    }

    /// Pops the top and then lets it move. (internal memory management use only)
    /// See docs at impl.
    #[inline]
    pub fn pop_top<F>(&self, f: F) -> MappingResult
        where F: FnOnce(PhysAddr) -> VirtAddr {
        self.allocator.lock().pop_top(f)
    }

    /// Similar to `pop_top`...
    /// See docs at impl.
    #[inline]
    pub fn push_top(&self, vaddr: VirtAddr, paddr: PhysAddr) {
        self.allocator.lock().push_top(vaddr, paddr)
    }
}

/// Gets the PMM.
pub fn get() -> &'static PhysMemManager {
    &PMM
}