use crate::arch::address::VirtAddr;
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

impl Thread {}

// TODO: bounds
impl Stack {
    /// Creates a new stack on a given location.
    pub unsafe fn new(location: VirtAddr) -> Self {
        Self { location }
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
