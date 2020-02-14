use alloc::collections::VecDeque;

use hashbrown::HashMap;

use crate::arch::address::VirtAddr;
use crate::arch::get_per_cpu_data;
use crate::mm::vma_allocator::Vma;
use crate::sync::spinlock::Spinlock;
use crate::tasking::thread::{Stack, Thread, ThreadId};
use crate::util::unchecked::UncheckedUnwrap;

// TODO: get rid of lookup in hashmap during critical path?

#[derive(Debug)]
#[repr(u64)]
pub enum SwitchReason {
    RegularSwitch = 0,
    Exit,
}

/// Common data for all per-core schedulers.
pub struct SchedulerCommon {
    threads: HashMap<ThreadId, Thread>,
}

/// Per-core scheduler.
pub struct Scheduler {
    // TODO: handle this so we can add without locking, currently this is no issue because you can only schedule on the current cpu
    runqueue: VecDeque<ThreadId>,
    garbage: Option<ThreadId>,
    current_thread_id: ThreadId,
    idle_thread_id: ThreadId,
}

impl SchedulerCommon {
    /// New scheduler common data.
    pub fn new() -> Self {
        Self {
            threads: HashMap::new(),
        }
    }

    /// Adds a thread.
    pub fn add_thread(&mut self, id: ThreadId, thread: Thread) {
        self.threads.insert(id, thread);
    }

    /// Removes a thread.
    pub fn remove_thread(&mut self, id: ThreadId) {
        self.threads.remove(&id);
    }

    /// Update thread state.
    pub fn update_thread_state(&mut self, id: ThreadId, stack: VirtAddr) {
        self.threads.get_mut(&id).unwrap().set_stack_address(stack);
    }

    /// Gets the thread stack address.
    pub fn get_thread_stack(&self, id: ThreadId) -> VirtAddr {
        self.threads.get(&id).unwrap().get_stack_address()
    }
}

impl Scheduler {
    /// New scheduler.
    fn new() -> Self {
        // This will be overwritten on the first context switch with valid data.
        let idle_thread = unsafe { Thread::new(Stack::new(Vma::empty())) };

        let idle_thread_id = ThreadId::new();
        with_common(|common| common.add_thread(idle_thread_id, idle_thread));

        Self {
            runqueue: VecDeque::new(),
            garbage: None,
            current_thread_id: idle_thread_id,
            idle_thread_id,
        }
    }

    /// Adds a thread to the runqueue.
    pub fn queue_thread(&mut self, id: ThreadId) {
        self.runqueue.push_back(id);
    }

    /// Gets the next thread id.
    fn next_thread_id(&mut self) -> ThreadId {
        // Decide next thread.
        if let Some(id) = self.runqueue.pop_front() {
            id
        } else {
            self.idle_thread_id
        }
    }

    /// Sets the scheduler up for switching to the next thread and gets the next thread stack address.
    pub fn next_thread_state(
        &mut self,
        switch_reason: SwitchReason,
        old_stack: VirtAddr,
    ) -> VirtAddr {
        // If we have lock now, it's a bad idea to switch. Postpone it instead.
        {
            let mut cpu_data = get_per_cpu_data();
            if unlikely!(cpu_data.scheduler_block_count != 0) {
                cpu_data.scheduler_postponed = true;
                return old_stack;
            }
        }

        let next_id = self.next_thread_id();
        let next_stack = with_common(|common| {
            // Cleanup old thread.
            if let Some(garbage) = self.garbage {
                common.remove_thread(garbage);
                self.garbage = None;
            }

            match switch_reason {
                // Regular switch.
                SwitchReason::RegularSwitch => {
                    common.update_thread_state(self.current_thread_id, old_stack);

                    if self.current_thread_id != self.idle_thread_id {
                        self.runqueue.push_back(self.current_thread_id);
                    }
                }
                // Exit the thread.
                SwitchReason::Exit => {
                    debug_assert!(self.garbage.is_none());
                    self.garbage = Some(self.current_thread_id);
                }
            }

            common.get_thread_stack(next_id)
        });

        self.current_thread_id = next_id;
        next_stack
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
pub fn add_and_schedule_thread(thread: Thread) -> ThreadId {
    let id = ThreadId::new();
    with_common(|common| common.add_thread(id, thread));
    with_core_scheduler(|scheduler| scheduler.queue_thread(id));
    id
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
