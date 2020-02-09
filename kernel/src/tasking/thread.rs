use core::mem::size_of;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{EntryFlags, PAGE_SIZE};
use crate::mm::mapper::MappingError;
use crate::mm::vma_allocator::with_vma_allocator;

/// The stack of a thread.
pub struct Stack {
    allocated_location: VirtAddr,
    current_location: VirtAddr,
    size: usize,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct ThreadId(u64);

impl ThreadId {
    /// Convert to 64-bit.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Create new thread id.
    pub fn new() -> Self {
        use core::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct Thread {
    stack: Stack,
}

impl Thread {
    /// Creates a thread.
    pub fn create(entry: VirtAddr) -> Result<Thread, MappingError> {
        // TODO
        let stack_size = 8 * PAGE_SIZE;
        let mut stack = Stack::create(stack_size)?;
        // Safe because enough size on the stack.
        unsafe {
            stack.prepare(entry);
        }
        Ok(Thread { stack })
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        with_vma_allocator(|vma| vma.free_region_and_unmap(self.allocated_location, self.size));
    }
}

// TODO: bounds
impl Stack {
    /// Creates a stack.
    pub fn create(size: usize) -> Result<Stack, MappingError> {
        let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
        let allocated_location = with_vma_allocator(|vma| vma.alloc_region_and_map(size, flags))?;
        let current_location = allocated_location + size;
        Ok(Stack {
            allocated_location,
            current_location,
            size,
        })
    }

    /// As a virtual address.
    #[inline]
    pub fn as_virt_addr(&self) -> VirtAddr {
        self.current_location
    } // TODO: remove me?

    /// Pushes a value on the stack.
    pub unsafe fn push<T>(&mut self, value: T) {
        self.current_location -= size_of::<T>();
        let ptr = self.current_location.as_mut();
        *ptr = value;
    }
}
