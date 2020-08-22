use crate::arch::asid::AsidManager;
use core::cell::{Cell, RefCell};

/// Per-CPU data.
#[repr(C, align(128))] // 128 = false sharing threshold
pub struct CpuData {
    /// Self reference.
    reference: usize,
    /// Preemption disable count. Zero means enabled.
    preempt_count: u32,
    /// Should schedule flag.
    should_schedule: u32,
    /// Address Space Identifier stuff.
    asid_enable: Cell<bool>,
    asid_manager: RefCell<AsidManager>,
}

impl CpuData {
    /// Creates a new empty per-CPU data.
    pub const fn new() -> Self {
        Self {
            // Need to fill in once we know the address.
            reference: 0,
            preempt_count: 0,
            should_schedule: 0,
            asid_enable: Cell::new(false),
            asid_manager: RefCell::new(AsidManager::new()),
        }
    }

    /// Offset of field `preempt_count`.
    pub const fn preempt_count_offset() -> usize {
        8
    }

    /// Gets the `preempt_count`.
    pub fn preempt_count(&self) -> u32 {
        self.preempt_count
    }

    /// Prepare to set the per-CPU data.
    pub fn prepare_to_set(&mut self, asid_enable: bool) {
        // Assembly code also trusts on this.
        assert_eq!(
            offset_of!(CpuData, preempt_count),
            Self::preempt_count_offset()
        );
        assert_eq!(offset_of!(CpuData, should_schedule), 12);
        assert_eq!(self.reference, 0);
        self.reference = self as *mut _ as usize;
        self.asid_enable.set(asid_enable);
    }

    /// Gets a mutable reference to the asid manager.
    pub fn asid_manager(&self) -> Option<&RefCell<AsidManager>> {
        self.asid_enable.get().then_some(&self.asid_manager)
    }
}
