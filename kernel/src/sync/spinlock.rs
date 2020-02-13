//! Spinlock, based on https://docs.rs/lock_api/0.3.3/lock_api/index.html, scheduler-aware.

use crate::sync::atomic_hle::AtomicHLE;
use core::sync::atomic::{spin_loop_hint, AtomicBool, AtomicU32, Ordering};
use lock_api::RawRwLock;
use lock_api::{GuardSend, Mutex, MutexGuard, RawMutex};

static TEST: AtomicU32 = AtomicU32::new(0);

pub struct RawSpinlock(AtomicBool);

unsafe impl RawMutex for RawSpinlock {
    const INIT: Self = Self(AtomicBool::new(false));
    type GuardMarker = GuardSend;

    fn lock(&self) {
        // We want to immediately try to acquire the lock, but if that fails, we want to use "test followed by test-and-set".
        // This is better for performance.
        while !self.try_lock() {
            // First check before trying again.
            // Relaxed ordering is for performance reasons. If it goes wrong, the `try_lock` will fail anyway.
            while self.0.load(Ordering::Relaxed) {
                spin_loop_hint();
            }
        }
    }

    fn try_lock(&self) -> bool {
        // If it is unlocked, it should currently have false as value.
        // For the success case, we don't want to have reordering, because that could cause data races.
        // For the failed case, we don't care because we didn't acquire the lock anyway.
        self.0
            .compare_exchange_acquire_relaxed_maybe_hle(false, true)
            .is_ok()
    }

    fn unlock(&self) {
        self.0.store_release_maybe_hle(false)
    }
}

pub type Spinlock<T> = Mutex<RawSpinlock, T>;
pub type SpinlockGuard<'a, T> = MutexGuard<'a, RawSpinlock, T>;
