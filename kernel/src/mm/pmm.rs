use multiboot2::BootInformation;

use crate::arch::address::{PhysAddr, VirtAddr};
use crate::mm::mapper::{MemoryError, MemoryResult};
use crate::sync::spinlock::Spinlock;
use core::intrinsics::unlikely;

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
/// So that means there is likely no extra TLB miss or cache miss.
pub struct FrameAllocator {
    pub top: PhysAddr,
}

impl FrameAllocator {
    /// Initializes the allocator.
    pub fn init(&mut self, mboot_struct: &BootInformation, reserved_end: PhysAddr) {
        let reserved_end = reserved_end.align_up();

        self.apply_mmap(
            mboot_struct
                .memory_map_tag()
                .expect("Memory map is required"),
            reserved_end,
        );
    }

    /// Empty, uninitialized allocator.
    const fn empty() -> Self {
        FrameAllocator {
            top: PhysAddr::null(),
        }
    }

    /// Pops the top and moves the current top pointer. This function is used internally for memory management by paging.
    pub fn pop_top<F>(&mut self, f: F) -> MemoryResult
    where
        F: FnOnce(PhysAddr) -> VirtAddr,
    {
        if unlikely(self.top.is_null()) {
            return Err(MemoryError::OOM);
        }

        // Read and set the next top address.
        let ptr = f(self.top).as_const();
        self.top = PhysAddr::new(unsafe { *ptr });
        Ok(())
    }

    /// Similar to `pop_top`.
    /// This pushes a new top on the stack and links it to the previous top.
    pub fn push_top(&mut self, vaddr: VirtAddr, paddr: PhysAddr) {
        let ptr: *mut usize = vaddr.as_mut();
        unsafe {
            ptr.write(self.top.as_usize());
        }
        self.top = paddr;
    }
}

static PMM: Spinlock<FrameAllocator> = Spinlock::new(FrameAllocator::empty());

/// Execute something using the PMM.
#[inline]
pub fn with_pmm<F, T>(f: F) -> T
where
    F: FnOnce(&mut FrameAllocator) -> T,
{
    f(&mut PMM.lock())
}
