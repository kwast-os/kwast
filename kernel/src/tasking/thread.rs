use core::mem::size_of;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{ActiveMapping, EntryFlags, PAGE_SIZE};
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::mm::vma_allocator::Vma;

/// The stack of a thread.
pub struct Stack {
    _vma: Vma,
    current_location: VirtAddr,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ThreadId(u64);

impl ThreadId {
    /// Create new thread id.
    pub fn new() -> Self {
        use core::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct Thread {
    stack: Stack,
}

impl Thread {
    /// Creates a thread.
    pub fn create(entry: VirtAddr) -> Result<Thread, MemoryError> {
        // TODO
        let stack_size = 8 * PAGE_SIZE;
        let mut stack = Stack::create(stack_size)?;
        // Safe because enough size on the stack.
        unsafe {
            stack.prepare(entry);
        }
        Ok(Self { stack })
    }

    /// Creates a new thread from given parameters.
    pub unsafe fn new(stack: Stack) -> Self {
        Self { stack }
    }

    /// Gets the current stack address.
    pub fn get_stack_address(&self) -> VirtAddr {
        self.stack.as_virt_addr()
    }

    /// Sets the current stack address.
    pub fn set_stack_address(&mut self, addr: VirtAddr) {
        self.stack.current_location = addr;
    }
}

// TODO: bounds
impl Stack {
    /// Creates a stack.
    pub fn create(size: usize) -> Result<Stack, MemoryError> {
        let vma = {
            let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
            let vma = Vma::create(size)?;
            ActiveMapping::get().map_range(vma.address(), size, flags)?;
            vma
        };
        Ok(Stack::new(vma))
    }

    /// Creates a new stack from given parameters.
    pub fn new(vma: Vma) -> Self {
        let current_location = vma.address() + vma.size();
        Self {
            _vma: vma,
            current_location,
        }
    }

    /// As a virtual address.
    #[inline]
    pub fn as_virt_addr(&self) -> VirtAddr {
        self.current_location
    }

    /// Pushes a value on the stack.
    pub unsafe fn push<T>(&mut self, value: T) {
        self.current_location -= size_of::<T>();
        let ptr = self.current_location.as_mut();
        *ptr = value;
    }
}
