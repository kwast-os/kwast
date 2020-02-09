use crate::arch::address::VirtAddr;
use crate::arch::x86_64::paging::PAGE_SIZE;
use crate::mm::mapper::MappingError;
use crate::mm::vma_allocator::with_vma_allocator;
use core::mem::size_of;

/// The stack of a thread.
pub struct Stack {
    location: VirtAddr,
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

impl Drop for Thread {
    fn drop(&mut self) {
        println!("drop thread");
        // TODO: free_region_and_unmap
        unimplemented!()
    }
}

// TODO: bounds
impl Stack {
    /// Creates a stack.
    pub fn create(size: usize) -> Result<Stack, MappingError> {
        let flags = EntryFlags::PRESENT | EntryFlags::WRITE | EntryFlags::NX;
        let location = with_vma_allocator(|vma| vma.alloc_region_and_map(size, flags))?;
        Ok(Stack { location })
    }

    /// As a virtual address.
    #[inline]
    pub fn as_virt_addr(&self) -> VirtAddr {
        self.location
    } // TODO: remove me?

    /// Pushes a value on the stack.
    pub unsafe fn push<T>(&mut self, value: T) {
        self.location -= size_of::<T>();
        let ptr = self.location.as_mut();
        *ptr = value;
    }
}
