/// Per-CPU data.
#[derive(Debug)]
#[repr(align(128))] // 128 = false sharing threshold
pub struct CpuData {
    /// Self reference.
    reference: usize,
    /// Counts how many times the scheduler is blocked at this moment for this core.
    /// This is designed to prevent switching while spinning.
    /// See locking & scheduler code.
    pub scheduler_block_count: u32,
    /// The scheduler postponed itself.
    pub scheduler_postponed: bool,
}

impl CpuData {
    /// Creates a new empty per-CPU data.
    pub const fn new() -> Self {
        Self {
            // Need to fill in once we know the address.
            reference: 0,
            scheduler_block_count: 0,
            scheduler_postponed: false,
        }
    }

    /// Prepare to set the per-CPU data.
    pub fn prepare_to_set(&mut self) {
        debug_assert_eq!(self.reference, 0);
        self.reference = self as *mut _ as usize;
    }
}
