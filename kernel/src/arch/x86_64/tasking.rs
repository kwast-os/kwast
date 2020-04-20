use crate::arch::address::VirtAddr;
use crate::tasking::thread::Stack;

impl Stack {
    /// Prepares the stack to execute the trampoline and go to `entry`.
    pub unsafe fn prepare_trampoline(&mut self, entry: VirtAddr, first_arg: usize) {
        extern "C" {
            fn thread_trampoline();
        }

        let rflags: u64 = (1 << 9) | (1 << 1);
        self.push(thread_trampoline as usize);
        self.push(rflags);
        self.push(entry.as_u64()); // rbx
        self.push(first_arg); // rbp
        self.push(0u64); // r12
        self.push(0u64); // r13
        self.push(0u64); // r14
        self.push(0u64); // r15
    }
}
