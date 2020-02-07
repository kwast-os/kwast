pub const HEAP_SIZE: u64 = 4 * 1024 * 1024; // 4 GiB
                                            // TODO: this should not be hardcoded, we should have an offset_of macro.
pub const HEAP_VMCTX_OFF: i32 = 0;

#[derive(Debug)]
pub struct VMContext {
    pub(crate) heap_base: usize,
}
