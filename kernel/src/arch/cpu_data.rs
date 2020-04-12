use crate::tasking::scheduler::{self, SwitchReason};
use core::cell::Cell;

/// Per-CPU data.
#[repr(C, align(128))] // 128 = false sharing threshold
#[derive(Debug)]
pub struct CpuData {
    /// Self reference.
    reference: usize,
    /// Preemption disable count. Zero means enabled.
    preempt_count: u32,
    /// Should schedule flag.
    should_schedule: Cell<u32>,
}

impl CpuData {
    /// Creates a new empty per-CPU data.
    pub const fn new() -> Self {
        Self {
            // Need to fill in once we know the address.
            reference: 0,
            preempt_count: 0,
            should_schedule: Cell::new(0),
        }
    }

    /// Offset of field `preempt_count`.
    pub const fn preempt_count_offset() -> usize {
        8
    }

    /// Should schedule now? Do if needed.
    pub fn check_should_schedule(&self) {
        if self.should_schedule.replace(0) != 0 {
            scheduler::switch_to_next(SwitchReason::RegularSwitch);
        }
    }

    /// Prepare to set the per-CPU data.
    pub fn prepare_to_set(&mut self) {
        // Assembly code also trusts on this.
        debug_assert_eq!(
            offset_of!(CpuData, preempt_count),
            Self::preempt_count_offset()
        );
        debug_assert_eq!(offset_of!(CpuData, should_schedule), 12);
        debug_assert_eq!(self.reference, 0);
        self.reference = self as *mut _ as usize;
    }
}
