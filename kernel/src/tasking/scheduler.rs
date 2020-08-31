use crate::arch::address::VirtAddr;
use crate::arch::get_per_cpu_data;
use crate::arch::paging::{get_cpu_page_mapping, CpuPageMapping};
use crate::mm::tcb_alloc::{tcb_alloc, tcb_dealloc, with_thread};
use crate::mm::vma_allocator::MappedVma;
use crate::sync::spinlock::Spinlock;
use crate::tasking::protection_domain::ProtectionDomain;
use crate::tasking::thread::{Stack, Thread, ThreadId, ThreadStatus};
use alloc::collections::VecDeque;
use atomic::Atomic;
use core::intrinsics::{likely, unlikely};
use core::sync::atomic::Ordering;
use spin::Once;

/// Per-core queues.
struct Queues {
    run_queue: VecDeque<ThreadId>,
}

/// Per-core scheduler.
pub struct Scheduler {
    queues: Spinlock<Queues>,
    garbage: Atomic<ThreadId>,
    current_thread_id: Atomic<ThreadId>,
    idle_thread_id: ThreadId,
}

impl Scheduler {
    /// New scheduler.
    fn new(idle_protection_domain: ProtectionDomain) -> Self {
        // This state will be overwritten on the first context switch with data from the current running code.
        let idle_thread = Thread::new(Stack::new(MappedVma::dummy()), idle_protection_domain);
        let idle_thread_id = idle_thread.id;
        tcb_alloc(idle_thread);

        Self {
            queues: Spinlock::new(Queues {
                run_queue: VecDeque::new(),
            }),
            garbage: Atomic::new(ThreadId::zero()),
            current_thread_id: Atomic::new(idle_thread_id),
            idle_thread_id,
        }
    }

    /// Adds a thread to the runqueue.
    pub fn queue_thread(&self, thread: ThreadId) {
        self.queues.lock().run_queue.push_back(thread);
    }

    /// Gets the next thread to run.
    #[inline]
    fn next_thread(&self, queues: &mut Queues) -> ThreadId {
        if let Some(thread) = queues.run_queue.pop_front() {
            thread
        } else {
            self.idle_thread_id
        }
    }

    /// Execute something with the current thread reference.
    pub fn with_current_thread<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&Thread) -> T,
    {
        with_thread(self.current_thread_id.load(Ordering::Acquire), f)
    }

    /// Gets the current thread id.
    #[inline]
    pub fn current_thread_id(&self) -> ThreadId {
        self.idle_thread_id
    }

    /// Moves a thread from the blocked queue to the runqueue if it was in the blocked queue.
    /// Returns true if it was in the blocked queue.
    pub(crate) fn move_wakeup(&self, thread_id: ThreadId) {
        let mut queues = self.queues.lock();
        queues.run_queue.push_front(thread_id);
    }

    /// Sets the scheduler up for switching to the next thread and gets the next thread stack address.
    fn next_thread_state(&self, old_stack: VirtAddr) -> NextThreadState {
        // Cleanup old thread.
        // Relaxed ordering is fine because this is only for this core.
        let garbage = self.garbage.load(Ordering::Relaxed);
        if unlikely(garbage != ThreadId::zero()) {
            tcb_dealloc(garbage);
            self.garbage.store(ThreadId::zero(), Ordering::Relaxed);
        }

        let mut queues = self.queues.lock();

        let old_thread_id = self.current_thread_id.load(Ordering::Acquire);

        let (old_mapping, old_thread_status) = with_thread(old_thread_id, |old_thread| {
            let old_thread_status = old_thread.status();

            if likely(!matches!(old_thread_status, ThreadStatus::Exit(_))) {
                old_thread.save_simd();
                old_thread.stack.set_current_location(old_stack);
            }

            (old_thread.domain().cpu_page_mapping(), old_thread_status)
        });

        match old_thread_status {
            ThreadStatus::Runnable => {
                if likely(old_thread_id != self.idle_thread_id) {
                    queues.run_queue.push_back(old_thread_id);
                }
            }

            ThreadStatus::Blocked => {}

            ThreadStatus::Exit(_) => {
                debug_assert_eq!(self.garbage.load(Ordering::Relaxed), ThreadId::zero());
                // Safety: We call this from an uninterrupted place and we are not referencing thread memory here.
                unsafe {
                    with_thread(old_thread_id, |old_thread| {
                        old_thread.unmap_memory();
                    });
                }
                self.garbage.store(old_thread_id, Ordering::Relaxed);
            }
        };

        /*print!("runqueue: ");
        for x in &queues.run_queue {
            print!("{:?} ", x.id);
        }
        println!();
        print!("blocked: ");
        for x in &queues.blocked_threads {
            print!("{:?} ", x.id);
        }
        println!();*/

        let next_thread_id = self.next_thread(&mut queues);
        debug_assert_eq!(
            { with_thread(next_thread_id, |next_thread| next_thread.status(),) },
            ThreadStatus::Runnable
        );

        self.current_thread_id
            .store(next_thread_id, Ordering::Release);

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
pub fn wakeup_and_yield(tid: ThreadId) {
    with_thread(tid, |t| t.wakeup());
    thread_yield();
}

/// Exit the thread.
pub fn thread_exit(exit_code: u32) -> ! {
    extern "C" {
        fn _thread_exit() -> !;
    }

    with_core_scheduler(|s| {
        s.with_current_thread(|thread| {
            assert_ne!(
                thread.id, s.idle_thread_id,
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

/// Adds and schedules a thread.
pub fn add_and_schedule_thread(thread: Thread) {
    let tid = thread.id;
    tcb_alloc(thread);
    with_core_scheduler(|scheduler| scheduler.queue_thread(tid));
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
    F: FnOnce(&Thread) -> T,
{
    with_core_scheduler(|s| s.with_current_thread(f))
}

/// Inits scheduler.
pub fn init() {
    SCHEDULER.call_once(|| {
        let idle_protection_domain =
            unsafe { ProtectionDomain::from_existing_mapping(get_cpu_page_mapping()) };
        Scheduler::new(idle_protection_domain)
    });
}
