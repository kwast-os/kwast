use alloc::collections::VecDeque;

use hashbrown::HashMap;
use spin::Mutex;

use crate::arch::address::VirtAddr;
use crate::tasking::thread::{Stack, Thread, ThreadId};
use crate::util::unchecked::UncheckedUnwrap;

pub struct Scheduler {
    threads: HashMap<ThreadId, Thread>,
    runlist: VecDeque<ThreadId>,
    current_thread_id: ThreadId,
    idle_thread_id: ThreadId,
}

impl Scheduler {
    /// New scheduler.
    fn new() -> Self {
        // This will be overwritten on the first context switch with valid data.
        let idle_thread = unsafe { Thread::new(Stack::new(VirtAddr::null(), 0)) };

        let idle_thread_id = ThreadId::new();
        let mut threads = HashMap::new();
        threads.insert(idle_thread_id, idle_thread);

        Self {
            threads,
            runlist: VecDeque::new(),
            current_thread_id: idle_thread_id,
            idle_thread_id,
        }
    }

    /// Adds a thread.
    pub fn add_thread(&mut self, id: ThreadId, thread: Thread) {
        self.threads.insert(id, thread);
        self.runlist.push_back(id);
    }

    /// Decides which thread to run next.
    fn next_thread(&mut self) -> ThreadId {
        if let Some(id) = self.runlist.pop_front() {
            id
        } else {
            self.idle_thread_id
        }
    }

    /// Sets the scheduler up for switching to the next thread and gets the next thread stack address.
    pub fn next_thread_state(&mut self, old_stack: VirtAddr) -> VirtAddr {
        self.threads
            .get_mut(&self.current_thread_id)
            .unwrap()
            .set_stack_address(old_stack);
        let next_id = self.next_thread();
        let next_stack = self.threads.get(&next_id).unwrap().get_stack_address();
        if self.current_thread_id != self.idle_thread_id {
            self.runlist.push_back(self.current_thread_id);
        }
        self.current_thread_id = next_id;
        next_stack
    }
}

/// Switches to the next thread.
#[inline]
pub fn switch_to_next() {
    extern "C" {
        fn _switch_to_next();
    }

    unsafe {
        _switch_to_next();
    }
}

/// Saves the old state and gets the next state.
#[no_mangle]
pub extern "C" fn next_thread_state(old_stack: VirtAddr) -> VirtAddr {
    with_scheduler(|scheduler| scheduler.next_thread_state(old_stack))
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
