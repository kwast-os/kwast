use crate::arch::address::VirtAddr;
use crate::arch::get_per_cpu_data;
use crate::arch::paging::{get_cpu_page_mapping, CpuPageMapping};
use crate::mm::vma_allocator::MappedVma;
use crate::sync::atomic::{AtomicArc, AtomicManagedPtr};
use crate::sync::spinlock::{RwLock, Spinlock};
use crate::tasking::protection_domain::ProtectionDomain;
use crate::tasking::thread::{Stack, Thread, ThreadId, ThreadStatus};
use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::sync::Arc;
use atomic::Atomic;
use core::intrinsics::{likely, unlikely};
use core::mem::ManuallyDrop;
use core::sync::atomic::Ordering;
use spin::Once;

/// Common data for all per-core schedulers.
pub struct SchedulerCommon {
    threads: BTreeMap<ThreadId, Arc<Thread>>,
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
            threads: BTreeMap::new(),
        }
    }

    /// Executes something in context of a thread.
    /// Executes `f` if the thread exists, `e` on error.
    pub fn with_thread<F, E, T>(&self, tid: ThreadId, f: F, e: E) -> T
    where
        F: FnOnce(&Arc<Thread>) -> T,
        E: FnOnce() -> T,
    {
        self.threads.get(&tid).map(f).unwrap_or_else(e)
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

    /// Wakes up a thread.
    pub fn wakeup(&self, id: ThreadId) -> bool {
        if let Some(thread) = self.threads.get(&id) {
            if thread
                .status_compare_exchange(
                    ThreadStatus::Blocked,
                    ThreadStatus::Runnable,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                // TODO: multicore
                return SCHEDULER
                    .try_get()
                    .expect("core scheduler for thread should exist")
                    .move_wakeup(id);
            }
        }

        false
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
            current_thread: AtomicManagedPtr::from(idle_thread.clone()),
            idle_thread,
        }
    }

    /// Adds a thread to the runqueue.
    pub fn queue_thread(&self, thread: Arc<Thread>) {
        self.queues.lock().run_queue.push_back(thread);
    }

    /// Gets the next thread to run.
    #[inline]
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
        // Safety:
        //
        // Loading would be unsafe if the Arc<Thread> instance is/becomes not valid during the
        // duration of this method.
        // We know that the strong count must be at least 2:
        //  * one in the common schedule structure
        //  * one because it's inside the current_thread.
        // We won't drop this, which keeps the strong count the same.
        //
        // If the one in the common schedule structure disappears, that means the thread will never
        // be scheduled again, so the current code path that holds the value won't be executed again.
        // So that case is fine.
        //
        // The only way the one from current_thread would be dropped is when the thread exits,
        // otherwise it's been moved to a runqueue / blockqueue / ...
        // If the thread exits, there is no problem for the same reason why the common schedule argument
        // holds: if the thread is exited, we won't be able to execute this path.
        //
        // This means the Arc<Thread> instance will never get strong count zero and be dropped
        // during the duration of this method.
        unsafe {
            f(&ManuallyDrop::new(
                self.current_thread.load(Ordering::Acquire),
            ))
        }
    }

    /// Moves a thread from the blocked queue to the runqueue if it was in the blocked queue.
    /// Returns true if it was in the blocked queue.
    fn move_wakeup(&self, thread_id: ThreadId) -> bool {
        let mut queues = self.queues.lock();

        if let Some(thread) = queues.blocked_threads.take(&thread_id) {
            queues.run_queue.push_front(thread);
            true
        } else {
            false
        }
    }

    /// Sets the scheduler up for switching to the next thread and gets the next thread stack address.
    fn next_thread_state(&self, old_stack: VirtAddr) -> NextThreadState {
        // Cleanup old thread.
        // Relaxed ordering is fine because this is only for this core.
        let garbage = self.garbage.load(Ordering::Relaxed);
        if unlikely(garbage != ThreadId::zero()) {
            with_common_mut(|common| common.remove_thread(garbage));
            self.garbage.store(ThreadId::zero(), Ordering::Relaxed);
        }

        let mut queues = self.queues.lock();

        // Problem: when there is only one thread to execute:
        //  * the queue is now empty
        //  * current_thread contains that only thread
        // Which means that if we were to safely swap, we would have nothing to swap to except
        // the idle thread.
        // Instead, we load the thread here.
        //
        // Safety:
        // This invalidates the contents of current_thread. We are not allowed to touch it until a
        // matching store happens (which we do later).
        let old_thread = unsafe { self.current_thread.load(Ordering::Acquire) };

        let old_mapping = old_thread.domain().cpu_page_mapping();

        let old_thread_status = old_thread.status();

        if likely(!matches!(old_thread_status, ThreadStatus::Exit(_))) {
            old_thread.save_simd();
            old_thread.stack.set_current_location(old_stack);
        }

        match old_thread_status {
            ThreadStatus::Runnable => {
                if likely(!Arc::ptr_eq(&old_thread, &self.idle_thread)) {
                    queues.run_queue.push_back(old_thread);
                }
            }

            ThreadStatus::Blocked => {
                queues.blocked_threads.insert(old_thread);
            }

            ThreadStatus::Exit(_) => {
                debug_assert_eq!(self.garbage.load(Ordering::Relaxed), ThreadId::zero());
                // Safety: We call this from an uninterrupted place and we are not referencing thread memory here.
                unsafe {
                    old_thread.unmap_memory();
                }
                self.garbage.store(old_thread.id(), Ordering::Relaxed);
            }
        };

        /*print!("runqueue: ");
        for x in &queues.run_queue {
            print!("{:?} ", x.id());
        }
        println!();
        print!("blocked: ");
        for x in &queues.blocked_threads {
            print!("{:?} ", x.id());
        }
        println!();*/

        let next_thread = self.next_thread(&mut queues);
        debug_assert_eq!(next_thread.status(), ThreadStatus::Runnable);

        // Safety:
        // This could lead to a memory leak if the old value is never read.
        // We did read the old value (it's in old_thread).
        // old_thread is either:
        //  * dropped like it should be in case of exit
        //  * added back to the runqueue, which means it's moved and won't be dropped here
        //  * inside of next_thread, which means it went through the runqueue and thus has been moved
        unsafe {
            self.current_thread.store(next_thread, Ordering::Release);
        }

        self.with_current_thread(|current_thread| {
            current_thread.restore_simd();
            let domain = current_thread.domain();
            domain.assign_asid_if_necessary();
            NextThreadState(current_thread.stack.get_current_location(), {
                let new_mapping = domain.cpu_page_mapping();
                if old_mapping == new_mapping {
                    CpuPageMapping::sentinel()
                } else {
                    new_mapping
                }
            })
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
    // If we manually switch and the `preempt_count` isn't zero, that indicates an issue in the code.
    debug_assert_eq!(
        get_per_cpu_data().preempt_count(),
        0,
        "trying to preempt while holding a spinlock"
    );

    switch_to_next();
}

/// Wakeup and yield.
pub fn wakeup_and_yield(id: ThreadId) {
    if with_common_scheduler(|s| s.wakeup(id)) {
        thread_yield();
    }
}

/// Exit the thread.
pub fn thread_exit(exit_code: u32) -> ! {
    extern "C" {
        fn _thread_exit() -> !;
    }

    with_core_scheduler(|s| {
        s.with_current_thread(|thread| {
            assert!(
                !Arc::ptr_eq(thread, &s.idle_thread),
                "Attempting to kill the idle thread"
            );
            thread.set_status(ThreadStatus::Exit(exit_code))
        })
    });
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
    f(&mut *SCHEDULER_COMMON
        .try_get()
        .expect("common scheduler")
        .write())
}

/// With common scheduler data. Read-only.
pub fn with_common_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&SchedulerCommon) -> T,
{
    f(&*SCHEDULER_COMMON.try_get().expect("common scheduler").read())
}

/// Execute something using this core-local scheduler.
pub fn with_core_scheduler<F, T>(f: F) -> T
where
    F: FnOnce(&Scheduler) -> T,
{
    f(&SCHEDULER.try_get().expect("core scheduler"))
}

/// Execute something using the current thread reference.
pub fn with_current_thread<F, T>(f: F) -> T
where
    F: FnOnce(&Arc<Thread>) -> T,
{
    with_core_scheduler(|s| s.with_current_thread(f))
}

/// Inits scheduler.
pub fn init() {
    SCHEDULER_COMMON.call_once(|| RwLock::new(SchedulerCommon::new()));
    SCHEDULER.call_once(|| {
        let idle_protection_domain =
            unsafe { ProtectionDomain::from_existing_mapping(get_cpu_page_mapping()) };
        Scheduler::new(idle_protection_domain)
    });
}
