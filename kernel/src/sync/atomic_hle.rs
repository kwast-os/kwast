//! Extends some atomic types to use hardware lock elision if possible.

use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;

pub trait AtomicHLE<T> {
    /// Compare exchange, with acquire ordering for success, and relaxed ordering for failure.
    /// May do hardware lock elision if possible.
    fn compare_exchange_acquire_relaxed_maybe_hle(&self, current: T, new: T) -> Result<T, T>;

    /// Atomic store with release ordering.
    /// May do hardware lock elision if possible.
    fn store_release_maybe_hle(&self, val: T);
}

impl AtomicHLE<bool> for AtomicBool {
    #[inline]
    fn compare_exchange_acquire_relaxed_maybe_hle(
        &self,
        current: bool,
        new: bool,
    ) -> Result<bool, bool> {
        if crate::arch::supports_hle() {
            // Safe because it uses the atomic instructions.
            unsafe {
                crate::arch::compare_exchange_acquire_relaxed_hle(self.as_mut_ptr(), current, new)
            }
        } else {
            self.compare_exchange(current, new, Ordering::Acquire, Ordering::Relaxed)
        }
    }

    #[inline]
    fn store_release_maybe_hle(&self, val: bool) {
        if crate::arch::supports_hle() {
            // Safe because it uses the atomic instructions.
            unsafe { crate::arch::store_release_hle(self.as_mut_ptr(), val) }
        } else {
            self.store(val, Ordering::Release)
        }
    }
}
