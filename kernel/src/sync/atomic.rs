use alloc::sync::Arc;
use core::sync::atomic::{AtomicPtr, Ordering};

/// Holds an Arc and supports swapping with other Arcs.
pub struct AtomicArc<T> {
    inner: AtomicPtr<T>,
}

#[allow(dead_code)]
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

    /// Stores a new Arc. This overwrites the old value, possibly leading to a memory leak if
    /// used without care. The caller should guarantee the use of this is safe.
    pub unsafe fn store(&self, arc: Arc<T>, ordering: Ordering) {
        assert_ne!(ordering, Ordering::Relaxed);

        // See comment from `from_arc`.
        self.inner.store(Arc::into_raw(arc) as *mut _, ordering);
    }
}
