use crate::arch::address::VirtAddr;
use core::mem::size_of;

pub struct Stack {
    location: VirtAddr,
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

    /// Prepares the stack to execute at `entry`.
    pub fn prepare(&mut self, entry: VirtAddr) {
        let rflags: u64 = (1 << 9) | (1 << 1);
        self.push(entry.as_u64());
        self.push(rflags);
        self.push(0u64); // rbx
        self.push(0u64); // rbp
        self.push(0u64); // r12
        self.push(0u64); // r13
        self.push(0u64); // r14
        self.push(0u64); // r15
    }
}
