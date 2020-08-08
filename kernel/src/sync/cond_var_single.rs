use crate::sync::spinlock::PreemptCounterInfluence;
use crate::tasking::scheduler;
use crate::tasking::scheduler::{with_common_scheduler, with_current_thread};
use crate::tasking::thread::{ThreadId, ThreadStatus};
use atomic::{Atomic, Ordering};
use spin::MutexGuard;

/// Simple version of a condition variable: single waiter, multiple notifiers.
/// There's no spurious wakeups.
pub struct CondVarSingle {
    waiter: Atomic<ThreadId>,
}

impl CondVarSingle {
    /// Creates a new `CondVarSingle`.
    pub fn new() -> Self {
        Self {
            waiter: Atomic::new(ThreadId::zero()),
        }
    }

    /// Notifies the waiter if there is one.
    pub fn notify(&self) {
        let tid = self.waiter.swap(ThreadId::zero(), Ordering::Acquire);
        if tid != ThreadId::zero() {
            with_common_scheduler(|s| {
                s.with_thread(tid, |scheduler, _thread| {
                    scheduler.wakeup_and_yield(tid);
                })
            });
        }
    }

    /// Wait until notified.
    pub fn wait<T>(&self, guard: MutexGuard<T, PreemptCounterInfluence>) {
        with_current_thread(|thread| {
            thread.set_status(ThreadStatus::Blocked);
            loop {
                match self.waiter.compare_exchange(
                    ThreadId::zero(),
                    thread.id(),
                    Ordering::Acquire,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(_) => continue,
                };
            }
            drop(guard);
        });
        scheduler::thread_yield();
    }
}

impl Drop for CondVarSingle {
    fn drop(&mut self) {
        self.notify();
    }
}
