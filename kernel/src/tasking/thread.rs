use crate::arch::address::VirtAddr;
use core::mem::size_of;

/// The stack of a thread.
pub struct Stack {
    location: VirtAddr,
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct ThreadId(u64);

pub struct Thread {
    stack: Stack,
}

impl Stack {
    /// Creates a new stack on a given location.
    pub unsafe fn new(location: VirtAddr) -> Self {
        Self { location }
    }

    /// As a virtual address.
    #[inline]
    pub fn as_virt_addr(&self) -> VirtAddr {
        self.location
    }

    /// Pushes a value on the stack.
    pub fn push<T>(&mut self, value: T) {
        self.location -= size_of::<T>();
        let ptr = self.location.as_mut();
        unsafe {
            *ptr = value;
        }
    }
}
