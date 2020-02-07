use crate::arch::address::VirtAddr;

extern "C" {
    /// Switch to a new stack.
    pub fn switch_to(new_stack: VirtAddr);
}
