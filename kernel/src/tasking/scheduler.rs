use alloc::collections::{BTreeSet, VecDeque};

use hashbrown::HashMap;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{get_cpu_page_mapping, CpuPageMapping};
use crate::mm::vma_allocator::MappedVma;
use crate::sync::spinlock::{RwLock, Spinlock};
use crate::tasking::protection_domain::ProtectionDomain;
use crate::tasking::thread::{Stack, Thread, ThreadId, ThreadStatus};
use alloc::sync::Arc;
use atomic::Atomic;
use bitflags::_core::sync::atomic::AtomicPtr;
use core::sync::atomic::Ordering;
use spin::Once;

/// Common data for all per-core schedulers.
pub struct SchedulerCommon {
    threads: HashMap<ThreadId, Arc<Thread>>,
}

/// Some magic to avoid a lock on a critical variable like current_thread.
struct AtomicArc<T> {
    inner: AtomicPtr<T>,
}

impl<T> AtomicArc<T> {
    /// Create from Arc<T>.
    pub fn from_arc(arc: Arc<T>) -> Self {
        Self {
            // AtomicPtr wants a *mut, but we only have *const.
            // We should remember to only use this as a *const.
            inner: AtomicPtr::new(Arc::into_raw(arc) as *mut _),
        }
    }

    /// Swaps with another Arc, returning the old value.
    pub fn swap(&self, other: Arc<T>, ordering: Ordering) -> Arc<T> {
        assert_ne!(ordering, Ordering::Relaxed);

        // See comment from `from_arc`.
        let old = self.inner.swap(Arc::into_raw(other) as *mut _, ordering) as *const _;

        // Safety:
        //  * Raw pointer was created previously by `Arc::into_raw` with the same type.
        //  * Only dropped once.
        unsafe { Arc::from_raw(old) }
    }

    /// Loads the value into an Arc. This does _not_ increase the strong count because it's not
    /// possible. The caller should guarantee the use of this is safe.
    ///
    /// This is unsafe in the general case because it allows for
    ///  * Dropping the same Arc<T> more than once.
    ///  * Get an invalid Arc<T> if the strong count is not at least zero during method execution and
    ///    use of the Arc<T>.
    ///  * Strong count is not increased.
    pub unsafe fn load(&self, ordering: Ordering) -> Arc<T> {
        assert_ne!(ordering, Ordering::Relaxed);

        // See comment from `from_arc`.
        let old = self.inner.load(ordering) as *const _;

        // The only safety here is that we know the raw pointer was created previously
        // by `Arc::into_raw` with the same type.
        Arc::from_raw(old)
    }
}

/// Per-core queues.
struct Queues {
    run_queue: VecDeque<Arc<Thread>>,
    blocked_threads: BTreeSet<Arc<Thread>>,
}

/// Per-core scheduler.
pub struct Scheduler {
    queues: Spinlock<Queues>,
    garbage: Atomic<ThreadId>,
    current_thread: AtomicArc<Thread>,
    idle_thread: Arc<Thread>,
}

impl SchedulerCommon {
    /// New scheduler common data.
    fn new() -> Self {
        Self {
            threads: HashMap::new(),
        }
    }

    /// Adds a thread.
    pub fn add_thread(&mut self, thread: Thread) -> Arc<Thread> {
        let id = thread.id();
        let thread = Arc::new(thread);
        self.threads.insert(id, thread.clone());
        thread
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
        let idle_thread = Thread::new(Stack::new(MappedVma::dummy()), idle_protection_domain);

        let idle_thread = with_common_mut(|common| common.add_thread(idle_thread));

        Self {
            queues: Spinlock::new(Queues {
                run_queue: VecDeque::new(),
                blocked_threads: BTreeSet::new(),
            }),
            garbage: Atomic::new(ThreadId::zero()),
            current_thread: AtomicArc::from_arc(idle_thread.clone()),
            idle_thread,
        }
    }

    /// Adds a thread to the runqueue.
    pub fn queue_thread(&self, thread: Arc<Thread>) {
        self.queues.lock().run_queue.push_back(thread);
    }

    /// Gets the next thread to run.
    fn next_thread(&self, queues: &mut Queues) -> Arc<Thread> {
        if let Some(thread) = queues.run_queue.pop_front() {
            thread
        } else {
            self.idle_thread.clone()
        }
    }

    /// Execute something with the current thread reference.
    pub fn with_current_thread<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&Arc<Thread>) -> T,
    {
        // TODO: explain safety
        unsafe {
            let arc = self.current_thread.load(Ordering::Acquire);
            let ret = f(&arc);
            core::mem::forget(arc);
            ret
        }
    }

    /// Wakes up a thread.
    /// Returns true if woken up.
    pub fn wakeup(&self, thread_id: ThreadId) -> bool {
        let mut queues = self.queues.lock();

        if let Some(thread) = queues.blocked_threads.take(&thread_id) {
            thread.set_status(ThreadStatus::Runnable);
            queues.run_queue.push_front(thread);
            true
        } else {
            false
        }
    }

    /// Wakes up a thread and switches to it.
    /// Returns true if woken up.
    pub fn wakeup_and_yield(&self, thread_id: ThreadId) -> bool {
        if self.wakeup(thread_id) {
            thread_yield();
            true
        } else {
            false
        }
    }

    /// Mark a thread as blocked.
    /// Will take effect next context switch.
    pub fn mark_as_blocked(&self) {
        self.with_current_thread(|thread| thread.set_status(ThreadStatus::Blocked));
    }

    /// Sets the scheduler up for switching to the next thread and gets the next thread stack address.
    fn next_thread_state(&self, old_stack: VirtAddr) -> NextThreadState {
        // Cleanup old thread.
        // Relaxed ordering is fine because this is only for this core.
        let garbage = self.garbage.load(Ordering::Relaxed);
        if garbage != ThreadId::zero() {
            with_common_mut(|common| common.remove_thread(garbage));
            self.garbage.store(ThreadId::zero(), Ordering::Relaxed);
        }

        let mut queues = self.queues.lock();

        // Decide which thread to run next.
        let old_thread = {
            // TODO: issue: can't run the same thread twice in a row
            let next_thread = self.next_thread(&mut queues);
            self.current_thread.swap(next_thread, Ordering::AcqRel)
        };

        let old_thread_status = old_thread.status();

        if !matches!(old_thread_status, ThreadStatus::Exit(_)) {
            old_thread.save_simd();
            old_thread.stack.set_current_location(old_stack);
        }

        match old_thread_status {
            ThreadStatus::Runnable => {
                if !Arc::ptr_eq(&old_thread, &self.idle_thread) {
                    queues.run_queue.push_back(old_thread);
                }
            }

            ThreadStatus::Blocked => {
                queues.blocked_threads.insert(old_thread);
            }

            ThreadStatus::Exit(_) => {
                debug_assert_eq!(self.garbage.load(Ordering::Relaxed), ThreadId::zero());
                unsafe {
                    // Safety: We call this from a safe place and we are not referencing thread memory here.
                    old_thread.unmap_memory();
                }
                self.garbage.store(old_thread.id(), Ordering::Relaxed);
            }
        };

        self.with_current_thread(|current_thread| {
            current_thread.restore_simd();
            let domain = current_thread.domain();
            domain.assign_asid_if_necessary();
            NextThreadState(
                current_thread.stack.get_current_location(),
                domain.cpu_page_mapping(),
            )
        })
    }
}

/// Switches to the next thread.
#[inline]
fn switch_to_next() {
    extern "C" {
        fn _switch_to_next();
    }

    unsafe {
        _switch_to_next();
    }
}

/// Yield the current thread.
#[inline]
pub fn thread_yield() {
    switch_to_next();
}

/// Mark current thread as blocked for the next context switch.
pub fn thread_mark_as_blocked() {
    with_core_scheduler(|s| s.mark_as_blocked());
}

/// Exit the thread.
#[inline]
pub fn thread_exit(exit_code: u32) -> ! {
    extern "C" {
        fn _thread_exit() -> !;
    }

    with_current_thread(|thread| thread.set_status(ThreadStatus::Exit(exit_code)));
    println!("thread exit: {}", exit_code);

    unsafe {
        _thread_exit();
    }
}

#[repr(C)]
struct NextThreadState(VirtAddr, CpuPageMapping);

/// Saves the old state and gets the next state.
#[no_mangle]
extern "C" fn next_thread_state(old_stack: VirtAddr) -> NextThreadState {
    with_core_scheduler(|scheduler| scheduler.next_thread_state(old_stack))
}

// TODO: make this per core once we go multicore
static SCHEDULER: Once<Scheduler> = Once::new();

static SCHEDULER_COMMON: Once<RwLock<SchedulerCommon>> = Once::new();

/// Adds and schedules a thread.
pub fn add_and_schedule_thread(thread: Thread) {
    let thread = with_common_mut(|common| common.add_thread(thread));
    with_core_scheduler(|scheduler| scheduler.queue_thread(thread));
}

/// With common scheduler data. Mutable.
fn with_common_mut<F, T>(f: F) -> T
where
    F: FnOnce(&mut SchedulerCommon) -> T,
{
    f(&mut *SCHEDULER_COMMON.call_once(scheduler_common_new).write())
}

/// With common scheduler data. Read-only.
fn with_common<F, T>(f: F) -> T
where
    F: FnOnce(&SchedulerCommon) -> T,
{
    f(&*SCHEDULER_COMMON.call_once(scheduler_common_new).read())
}

/// Execute something using this core scheduler.
pub fn with_core_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&Scheduler) -> T,
{
    // This is local to the current core.
    f(&SCHEDULER.call_once(scheduler_core_new))
}

/// Execute something using the current thread reference.
pub fn with_current_thread<F, T>(f: F) -> T
where
    F: FnOnce(&Arc<Thread>) -> T,
{
    with_core_scheduler(|s| s.with_current_thread(f))
}

/// Factory for scheduler common inner type.
fn scheduler_common_new() -> RwLock<SchedulerCommon> {
    RwLock::new(SchedulerCommon::new())
}

/// Factory for scheduler common inner type.
fn scheduler_core_new() -> Scheduler {
    let idle_protection_domain =
        unsafe { ProtectionDomain::from_existing_mapping(get_cpu_page_mapping()) };
    Scheduler::new(idle_protection_domain)
}

/// Inits scheduler.
pub fn init() {
    SCHEDULER_COMMON.call_once(scheduler_common_new);
    SCHEDULER.call_once(scheduler_core_new);
}
