use crate::sync::cond_var_single::CondVarSingle;
use crate::sync::spinlock::Spinlock;
use alloc::collections::VecDeque;
use bitflags::_core::intrinsics::unlikely;

/// A queue with one waiter and multiple producers.
pub struct WaitQueue<T> {
    queue: Spinlock<VecDeque<T>>,
    cond_var: CondVarSingle,
}

impl<T> WaitQueue<T> {
    /// Creates a new `WaitQueue`.
    pub fn new() -> Self {
        Self {
            queue: Spinlock::new(VecDeque::new()),
            cond_var: CondVarSingle::new(),
        }
    }

    /// Appends an element to the back.
    /// Notifies the waiter.
    pub fn push_back(&self, t: T) {
        self.queue.lock().push_back(t);
        println!("push");
        self.cond_var.notify();
    }

    /// Pops an element from the front.
    /// Waits if no elements are available.
    pub fn pop_front(&self) -> T {
        loop {
            let mut guard = self.queue.lock();
            if let Some(t) = guard.pop_front() {
                return t;
            } else {
                self.cond_var.wait(guard);
            }
        }
    }

    /// If there are no elements available: block.
    /// Otherwise: pops as many elements as possible without going to block.
    pub fn pop_front_many(&self, buffer: &mut [T]) -> usize {
        if unlikely(buffer.is_empty()) {
            return 0;
        }

        //let guard = self.queue.lock();
        //println!("pop_front_many: {}", guard.len());
        let mut count = 0;
        //if guard.is_empty() {
        //    drop(guard);
        //    buffer[count] = self.pop_front();
        //    count += 1;
        //} else {
        //    drop(guard);
        //}
        buffer[count] = self.pop_front();
        count += 1;

        let mut guard = self.queue.lock();
        while !guard.is_empty() && count < buffer.len() {
            buffer[count] = guard.pop_front().expect("is not empty");
            count += 1;
        }

        count
    }
}
