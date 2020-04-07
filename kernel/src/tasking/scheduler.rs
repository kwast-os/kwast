use alloc::collections::VecDeque;

use hashbrown::HashMap;

use crate::arch::address::VirtAddr;
use crate::mm::vma_allocator::{LazilyMappedVma, MappedVma};
use crate::sync::spinlock::Spinlock;
use crate::tasking::thread::{Stack, Thread, ThreadId};
use crate::util::unchecked::UncheckedUnwrap;
use alloc::sync::Arc;
use core::mem::swap;

#[derive(Debug, PartialEq)]
#[repr(u64)]
pub enum SwitchReason {
    RegularSwitch = 0,
    Exit = 1,
}

/// Common data for all per-core schedulers.
pub struct SchedulerCommon {
    threads: HashMap<ThreadId, Arc<Thread>>,
}

/// Per-core scheduler.
pub struct Scheduler {
    // TODO: handle this so we can add without locking the whole scheduler, currently this is no issue because you can only schedule on the current cpu
    run_queue: VecDeque<Arc<Thread>>,
    garbage: Option<ThreadId>,
    current_thread: Arc<Thread>,
    idle_thread: Arc<Thread>,
}

impl SchedulerCommon {
    /// New scheduler common data.
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
        }
    }

    /// Adds a thread.
    pub fn add_thread(&mut self, thread: Arc<Thread>) {
        self.threads.insert(thread.id(), thread);
    }

    /// Removes a thread.
    pub fn remove_thread(&mut self, id: ThreadId) {
        self.threads.remove(&id);
    }
}

impl Scheduler {
    /// New scheduler.
    fn new() -> Self {
        // This will be overwritten on the first context switch with data from the current running code.
        let idle_thread = Arc::new(Thread::new(
            Stack::new(MappedVma::dummy()),
            MappedVma::dummy(),
            LazilyMappedVma::dummy(),
            None,
        ));

        with_common(|common| common.add_thread(idle_thread.clone()));

        Self {
            run_queue: VecDeque::new(),
            garbage: None,
            current_thread: idle_thread.clone(),
            idle_thread,
        }
    }

    /// Adds a thread to the runqueue.
    pub fn queue_thread(&mut self, thread: Arc<Thread>) {
        self.run_queue.push_back(thread);
    }

    /// Gets the next thread to run.
    fn next_thread(&mut self) -> Arc<Thread> {
        if let Some(thread) = self.run_queue.pop_front() {
            thread
        } else {
            self.idle_thread.clone()
        }
    }

    /// Gets the current thread.
    pub fn get_current_thread(&self) -> &Arc<Thread> {
        &self.current_thread
    }

    /// Sets the scheduler up for switching to the next thread and gets the next thread stack address.
    pub fn next_thread_state(
        &mut self,
        switch_reason: SwitchReason,
        old_stack: VirtAddr,
    ) -> VirtAddr {
        // Cleanup old thread.
        if let Some(garbage) = self.garbage {
            with_common(|common| common.remove_thread(garbage));
            self.garbage = None;
        }

        // Decide which thread to run next.
        let old_thread = {
            let mut next_thread = self.next_thread();
            swap(&mut self.current_thread, &mut next_thread);
            next_thread
        };

        match switch_reason {
            // Regular switch.
            SwitchReason::RegularSwitch => {
                old_thread.stack.set_current_location(old_stack);

                if !Arc::ptr_eq(&old_thread, &self.idle_thread) {
                    self.run_queue.push_back(old_thread);
                }
            }
            // Exit the thread.
            SwitchReason::Exit => {
                debug_assert!(self.garbage.is_none());
                self.garbage = Some(old_thread.id());
            }
        }

        self.current_thread.stack.get_current_location()
    }
}

/// Switches to the next thread.
#[inline]
pub fn switch_to_next(switch_reason: SwitchReason) {
    extern "C" {
        fn _switch_to_next(switch_reason: SwitchReason);
    }

    unsafe {
        _switch_to_next(switch_reason);
    }
}

/// Saves the old state and gets the next state.
#[no_mangle]
pub extern "C" fn next_thread_state(switch_reason: SwitchReason, old_stack: VirtAddr) -> VirtAddr {
    with_core_scheduler(|scheduler| scheduler.next_thread_state(switch_reason, old_stack))
}

// TODO: make this per core once we go multicore
static mut SCHEDULER: Option<Scheduler> = None;

// TODO: RwLock instead of Spinlock?
static mut SCHEDULER_COMMON: Spinlock<Option<SchedulerCommon>> = Spinlock::new(None);

/// Adds and schedules a thread.
pub fn add_and_schedule_thread(thread: Thread) {
    let thread = Arc::new(thread);
    with_common(|common| common.add_thread(thread.clone()));
    with_core_scheduler(|scheduler| scheduler.queue_thread(thread));
}

/// With common scheduler data.
fn with_common<F, T>(f: F) -> T
where
    F: FnOnce(&mut SchedulerCommon) -> T,
{
    unsafe { f(SCHEDULER_COMMON.lock().as_mut().unchecked_unwrap()) }
}

/// Execute something using this core scheduler.
pub fn with_core_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&mut Scheduler) -> T,
{
    unsafe { f(SCHEDULER.as_mut().unchecked_unwrap()) }
}

/// Inits scheduler. May only be called once per core.
pub unsafe fn init() {
    debug_assert!(SCHEDULER.is_none());
    *SCHEDULER_COMMON.lock() = Some(SchedulerCommon::new());
    SCHEDULER = Some(Scheduler::new());
}
