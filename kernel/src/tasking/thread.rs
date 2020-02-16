use core::mem::size_of;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{EntryFlags, PAGE_SIZE};
use crate::mm::mapper::MemoryError;
use crate::mm::vma_allocator::{MappedVma, Vma};

/// The stack of a thread.
pub struct Stack {
    _vma: MappedVma,
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
        let stack_guard_size: usize = PAGE_SIZE;
        let mut stack = Stack::create(stack_size, stack_guard_size)?;
        println!("{:?}", stack._vma.address());
        // Safe because enough size on the stack and stack allocated at a known good location.
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

impl Stack {
    /// Creates a stack.
    pub fn create(size: usize, guard_size: usize) -> Result<Stack, MemoryError> {
        let vma = {
            let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
            Vma::create(size + guard_size)?.map(guard_size, size, flags)?
        };
        Ok(Stack::new(vma))
    }

    /// Creates a new stack from given parameters.
    pub fn new(vma: MappedVma) -> Self {
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
