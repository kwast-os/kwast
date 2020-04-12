use crate::arch::interrupts::{irq_restore, irq_save_and_stop, IrqState};
use crate::arch::{get_per_cpu_data, preempt_disable, preempt_enable};
use spin::{Mutex, SchedulerInfluence};

pub struct PreemptCounterInfluence {}

pub struct IrqInfluence {
    state: IrqState,
}

impl SchedulerInfluence for PreemptCounterInfluence {
    #[inline(always)]
    fn preempt_enable(&self) {
        preempt_enable();
    }

    #[inline(always)]
    fn preempt_disable() -> Self {
        preempt_disable();
        Self {}
    }

    #[inline(always)]
    fn check_schedule_flag() {
        get_per_cpu_data().check_should_schedule();
    }
}

impl SchedulerInfluence for IrqInfluence {
    fn preempt_enable(&self) {
        irq_restore(self.state)
    }

    fn preempt_disable() -> Self {
        Self {
            state: irq_save_and_stop(),
        }
    }

    fn check_schedule_flag() {}
}

// TODO: apply Hardware Lock Elision if supported

pub type Spinlock<T> = Mutex<T, PreemptCounterInfluence>;

pub type IrqSpinlock<T> = Mutex<T, IrqInfluence>;
