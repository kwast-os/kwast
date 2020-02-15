//! Spinlock, based on https://docs.rs/lock_api/0.3.3/lock_api/index.html, scheduler-aware.

use crate::arch;
use crate::sync::atomic_hle::AtomicHLE;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{spin_loop_hint, AtomicBool, Ordering};

pub struct Spinlock<T: ?Sized> {
    flag: AtomicBool,
    data: UnsafeCell<T>,
}

pub struct SpinlockGuard<'a, T: ?Sized + 'a> {
    lock: &'a Spinlock<T>,
    state: arch::IrqState,
}

unsafe impl<T: ?Sized + Send> Send for Spinlock<T> {}
unsafe impl<T: ?Sized + Send> Sync for Spinlock<T> {}

impl<T> Spinlock<T> {
    /// Create a new spinlock.
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
            flag: AtomicBool::new(false),
        }
    }

    /// Lock.
    pub fn lock(&self) -> SpinlockGuard<T> {
        let state = arch::irq_save_and_stop();

        // We want to immediately try to acquire the lock, but if that fails, we want to use "test followed by test-and-set".
        // This is better for performance.
        while !self.try_set_lock_flag() {
            // First check before trying again.
            // Relaxed ordering is for performance reasons. If it goes wrong, the `try_lock` will fail anyway.
            while self.flag.load(Ordering::Relaxed) {
                spin_loop_hint();
            }
        }

        SpinlockGuard { lock: self, state }
    }

    /// Try setting the lock flag.
    fn try_set_lock_flag(&self) -> bool {
        // If it is unlocked, it should currently have false as value.
        // For the success case, we don't want to have reordering, because that could cause data races.
        // For the failed case, we don't care because we didn't acquire the lock anyway.
        self.flag
            .compare_exchange_acquire_relaxed_maybe_hle(false, true)
            .is_ok()
    }

    /// Try locking.
    #[allow(dead_code)]
    pub fn try_lock(&self) -> Option<SpinlockGuard<T>> {
        let state = arch::irq_save_and_stop();

        if self.try_set_lock_flag() {
            Some(SpinlockGuard { lock: self, state })
        } else {
            arch::irq_restore(state);
            None
        }
    }
}

impl<'a, T: ?Sized> Drop for SpinlockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.flag.store_release_maybe_hle(false);
        arch::irq_restore(self.state);
    }
}

impl<'a, T: ?Sized> Deref for SpinlockGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> DerefMut for SpinlockGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}
