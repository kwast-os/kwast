use crate::arch::address::VirtAddr;
use crate::tasking::thread::Stack;
use crate::wasm::vmctx::VmContext;

impl Stack {
    /// Prepares the stack to execute the trampoline and go to `entry`.
    pub unsafe fn prepare_trampoline(&mut self, entry: VirtAddr, vmctx: *const VmContext) {
        extern "C" {
            fn wasm_trampoline();
        }

        let rflags: u64 = (1 << 9) | (1 << 1);
        self.push(wasm_trampoline as usize);
        self.push(rflags);
        self.push(entry.as_u64()); // rbx
        self.push(vmctx as u64); // rbp
        self.push(0u64); // r12
        self.push(0u64); // r13
        self.push(0u64); // r14
        self.push(0u64); // r15
    }
}
