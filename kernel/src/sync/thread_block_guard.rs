use crate::tasking::scheduler;
use crate::tasking::scheduler::with_current_thread;
use crate::tasking::thread::ThreadStatus;
use core::intrinsics::likely;

/// Guard that marks the thread as blocked.
/// The thread will be yielded and woken up later on drop if the resource hasn't become available
/// already.
/// If it's become available already, no yield will happen and the thread can continue immediately.
pub struct ThreadBlockGuard {}

impl ThreadBlockGuard {
    /// Activates the block guard.
    pub fn activate() -> Self {
        // Mark the thread as blocked.
        // Next context switch the thread will block.
        with_current_thread(|thread| thread.set_status(ThreadStatus::Blocked));
        Self {}
    }
}

impl Drop for ThreadBlockGuard {
    fn drop(&mut self) {
        // It is possible (although very unlikely) that we don't have to block anymore
        // because what we block on has become available already.
        // The scheduler will have marked the thread as `Runnable` again in that case.
        // We don't have to yield in that case.
        // TL;DR: if it's still blocked: yield.
        if likely(with_current_thread(|thread| thread.status()) == ThreadStatus::Blocked) {
            scheduler::thread_yield();
        }
    }
}
