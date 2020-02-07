use crate::tasking::thread::{Thread, ThreadId};
use crate::util::unchecked::UncheckedUnwrap;
use alloc::collections::VecDeque;
use hashbrown::HashMap;
use spin::Mutex;

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
}

static SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);

/// Execute something using the scheduler.
pub fn with_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&mut Scheduler) -> T,
{
    unsafe { f(SCHEDULER.lock().as_mut().unchecked_unwrap()) }
}

/// Inits scheduler.
pub fn init() {
    debug_assert!(SCHEDULER.lock().is_none());
    *SCHEDULER.lock() = Some(Scheduler::new());
}
