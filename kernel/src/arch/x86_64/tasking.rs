use crate::arch::address::VirtAddr;
use crate::tasking::Stack;

impl Stack {
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
