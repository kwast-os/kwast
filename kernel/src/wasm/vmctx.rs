use crate::arch::address::VirtAddr;
use crate::arch::paging::PAGE_SIZE;

pub const HEAP_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4 GiB

pub const HEAP_GUARD_SIZE: u64 = 1 * PAGE_SIZE as u64;

// TODO: this should not be hardcoded, we should have an offset_of macro.
pub const HEAP_VMCTX_OFF: i32 = 0;

#[derive(Debug)]
pub struct VMContext {
    pub(crate) heap_base: VirtAddr,
}
