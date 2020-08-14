use crate::arch::interrupts::{irq_restore, irq_save_and_stop, IrqState};
use crate::arch::{check_should_schedule, preempt_disable, preempt_enable};
use spin::{self, SchedulerInfluence};

pub struct PreemptCounterInfluence {}

pub struct IrqInfluence {
    state: IrqState,
}

impl SchedulerInfluence for PreemptCounterInfluence {
    #[inline(always)]
    fn activate() -> Self {
        preempt_disable();
        Self {}
    }
}

impl Drop for PreemptCounterInfluence {
    #[inline]
    fn drop(&mut self) {
        preempt_enable();
        check_should_schedule();
    }
}

impl SchedulerInfluence for IrqInfluence {
    fn activate() -> Self {
        Self {
            state: irq_save_and_stop(),
        }
    }
}

impl Drop for IrqInfluence {
    #[inline]
    fn drop(&mut self) {
        irq_restore(self.state);
    }
}

// TODO: apply Hardware Lock Elision if supported

pub type Spinlock<T> = spin::Mutex<T, PreemptCounterInfluence>;
pub type RwLock<T> = spin::RwLock<T, PreemptCounterInfluence>;
pub type IrqSpinlock<T> = spin::Mutex<T, IrqInfluence>;
