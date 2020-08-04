use alloc::collections::{BTreeSet, VecDeque};

use hashbrown::HashMap;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{get_cpu_page_mapping, CpuPageMapping};
use crate::mm::vma_allocator::MappedVma;
use crate::sync::spinlock::RwLock;
use crate::tasking::protection_domain::ProtectionDomain;
use crate::tasking::thread::{Stack, Thread, ThreadId};
use crate::util::unchecked::UncheckedUnwrap;
use alloc::sync::Arc;
use core::mem::swap;

#[derive(Debug, PartialEq)]
#[repr(u64)]
#[allow(dead_code)]
enum SwitchReason {
    RegularSwitch = 0,
    Exit = 1,
    Block = 2,
}

/// Common data for all per-core schedulers.
pub struct SchedulerCommon {
    threads: HashMap<ThreadId, Arc<Thread>>,
}

/// Per-core scheduler.
pub struct Scheduler {
    // TODO: handle this so we can add without locking the whole scheduler, currently this is no issue because you can only schedule on the current cpu
    run_queue: VecDeque<Arc<Thread>>,
    blocked_threads: BTreeSet<Arc<Thread>>,
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
    fn new(idle_protection_domain: ProtectionDomain) -> Self {
        // This will be overwritten on the first context switch with data from the current running code.
        let idle_thread = Arc::new(Thread::new(
            Stack::new(MappedVma::dummy()),
            idle_protection_domain,
        ));

        with_common_mut(|common| common.add_thread(idle_thread.clone()));

        Self {
            run_queue: VecDeque::new(),
            blocked_threads: BTreeSet::new(),
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

    /// Wakes up a thread.
    /// Returns true if woken up.
    pub fn wakeup(&mut self, thread_id: ThreadId) -> bool {
        if let Some(thread) = self.blocked_threads.take(&thread_id) {
            self.run_queue.push_front(thread);
            true
        } else {
            false
        }
    }

    /// Wakes up a thread and switches to it.
    /// Returns true if woken up.
    pub fn wakeup_and_yield(&mut self, thread_id: ThreadId) -> bool {
        if self.wakeup(thread_id) {
            thread_yield();
            true
        } else {
            false
        }
    }

    /// Sets the scheduler up for switching to the next thread and gets the next thread stack address.
    fn next_thread_state(
        &mut self,
        switch_reason: SwitchReason,
        old_stack: VirtAddr,
    ) -> NextThreadState {
        // Cleanup old thread.
        if let Some(garbage) = self.garbage {
            with_common_mut(|common| common.remove_thread(garbage));
            self.garbage = None;
        }

        // Decide which thread to run next.
        let old_thread = {
            let mut next_thread = self.next_thread();
            swap(&mut self.current_thread, &mut next_thread);
            next_thread
        };

        if switch_reason != SwitchReason::Exit {
            old_thread.save_simd();
            old_thread.stack.set_current_location(old_stack);
        }

        match switch_reason {
            SwitchReason::RegularSwitch => {
                if !Arc::ptr_eq(&old_thread, &self.idle_thread) {
                    self.run_queue.push_back(old_thread);
                }
            }

            SwitchReason::Block => {
                self.blocked_threads.insert(old_thread);
            }

            SwitchReason::Exit => {
                debug_assert!(self.garbage.is_none());
                unsafe {
                    // Safety: We call this from a safe place and we are not referencing thread memory here.
                    old_thread.unmap_memory();
                }
                self.garbage = Some(old_thread.id());
            }
        }

        self.current_thread.restore_simd();
        let domain = self.get_current_thread().domain();
        domain.assign_asid_if_necessary();
        NextThreadState(
            self.current_thread.stack.get_current_location(),
            domain.cpu_page_mapping(),
        )
    }
}

/// Switches to the next thread.
#[inline]
fn switch_to_next(switch_reason: SwitchReason) {
    extern "C" {
        fn _switch_to_next(switch_reason: SwitchReason);
    }

    unsafe {
        _switch_to_next(switch_reason);
    }
}

/// Yield the current thread.
#[inline]
pub fn thread_yield() {
    switch_to_next(SwitchReason::RegularSwitch);
}

/// Blocks the current thread.
#[inline]
pub fn thread_block() {
    switch_to_next(SwitchReason::Block);
}

/// Exit the thread.
#[inline]
pub fn thread_exit(exit_code: u32) -> ! {
    extern "C" {
        fn _thread_exit() -> !;
    }

    println!("thread exit: {}", exit_code);

    unsafe {
        _thread_exit();
    }
}

#[repr(C)]
struct NextThreadState(VirtAddr, CpuPageMapping);

/// Saves the old state and gets the next state.
#[no_mangle]
extern "C" fn next_thread_state(
    switch_reason: SwitchReason,
    old_stack: VirtAddr,
) -> NextThreadState {
    with_core_scheduler(|scheduler| scheduler.next_thread_state(switch_reason, old_stack))
}

// TODO: make this per core once we go multicore
static mut SCHEDULER: Option<Scheduler> = None;

static mut SCHEDULER_COMMON: RwLock<Option<SchedulerCommon>> = RwLock::new(None);

/// Adds and schedules a thread.
pub fn add_and_schedule_thread(thread: Thread) {
    let thread = Arc::new(thread);
    with_common_mut(|common| common.add_thread(thread.clone()));
    with_core_scheduler(|scheduler| scheduler.queue_thread(thread));
}

/// With common scheduler data. Mutable.
fn with_common_mut<F, T>(f: F) -> T
where
    F: FnOnce(&mut SchedulerCommon) -> T,
{
    unsafe { f(SCHEDULER_COMMON.write().as_mut().unchecked_unwrap()) }
}

/// With common scheduler data. Read-only.
fn with_common<F, T>(f: F) -> T
where
    F: FnOnce(&SchedulerCommon) -> T,
{
    unsafe { f(SCHEDULER_COMMON.read().as_ref().unchecked_unwrap()) }
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
    *SCHEDULER_COMMON.write() = Some(SchedulerCommon::new());
    let idle_protection_domain = ProtectionDomain::from_existing_mapping(get_cpu_page_mapping());
    SCHEDULER = Some(Scheduler::new(idle_protection_domain));
}
