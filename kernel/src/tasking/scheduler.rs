use alloc::collections::VecDeque;

use hashbrown::HashMap;
use spin::Mutex;

use crate::arch::x86_64::address::VirtAddr;
use crate::tasking::thread::{Thread, ThreadId};
use crate::util::unchecked::UncheckedUnwrap;

pub struct Scheduler {
    threads: HashMap<ThreadId, Thread>,
    runlist: VecDeque<ThreadId>,
}

impl Scheduler {
    /// New scheduler.
    fn new() -> Self {
        Self {
            threads: HashMap::new(),
            runlist: VecDeque::new(),
        }
    }

    /// Adds a thread.
    pub fn add_thread(&mut self, id: ThreadId, thread: Thread) {
        self.threads.insert(id, thread);
        self.runlist.push_back(id);
    }
}

extern "C" {
    /// Switch to a new stack.
    pub fn switch_to(new_stack: VirtAddr);
}

static SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);

/// Execute something using the scheduler.
pub fn with_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&mut Scheduler) -> T,
{
    unsafe { f(SCHEDULER.lock().as_mut().unchecked_unwrap()) }
}

/// Inits scheduler. May only be called once per core.
pub unsafe fn init() {
    debug_assert!(SCHEDULER.lock().is_none());
    *SCHEDULER.lock() = Some(Scheduler::new());
}
