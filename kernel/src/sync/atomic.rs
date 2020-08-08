#![allow(dead_code)]

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::marker::PhantomData;
use core::ptr::null;
use core::sync::atomic::{AtomicPtr, Ordering};

/// Holds a managed pointer and supports swapping with other managed pointers of the same type.
/// Caller is responsible for handling memory leaks.
pub struct AtomicManagedPtr<T, C: IntoRawPtr<T> + FromRawPtr<T>> {
    inner: AtomicPtr<C>,
    _phantom: PhantomData<fn(T) -> T>, // same variance as T, but Send + Sync
}

pub type AtomicArc<T> = AtomicManagedPtr<T, Arc<T>>;
pub type AtomicBox<T> = AtomicManagedPtr<T, Box<T>>;
pub type AtomicOptionBox<T> = AtomicManagedPtr<T, Option<Box<T>>>;

pub trait IntoRawPtr<T> {
    fn into_raw(self) -> *const T;
}

pub trait FromRawPtr<T> {
    unsafe fn from_raw(t: *const T) -> Self;
}

impl<T, C: IntoRawPtr<T> + FromRawPtr<T>> AtomicManagedPtr<T, C> {
    /// Create from `C`.
    pub fn from(from: C) -> Self {
        Self {
            // AtomicPtr wants a *mut, but we only have *const.
            // We should remember to only use this as a *const.
            inner: AtomicPtr::new(C::into_raw(from) as *mut _),
            _phantom: PhantomData,
        }
    }

    /// Assert ordering not one of the invalid ones.
    fn assert_ordering(ordering: Ordering) {
        // Relaxed allows "out of thin air" values if used without care.
        assert_ne!(ordering, Ordering::Relaxed);
    }

    /// Swaps with another `C`s, returning the old value.
    pub fn swap(&self, other: C, ordering: Ordering) -> C {
        Self::assert_ordering(ordering);

        // See comment from `from`.
        let old = self.inner.swap(C::into_raw(other) as *mut _, ordering) as *const _;

        // Safety:
        //  * Raw pointer was created previously by `C::into_raw` with the same type.
        //  * Only dropped once.
        unsafe { C::from_raw(old) }
    }

    /// Loads the value into a `C`. This does _not_ increase the strong count because it's not
    /// possible (in case of `Arc`). The caller should guarantee the use of this is safe.
    ///
    /// This is unsafe in the general case because:
    ///  * It allows dropping the same `C` more than once.
    ///  * Strong count is not increased (in case of Arc).
    pub unsafe fn load(&self, ordering: Ordering) -> C {
        Self::assert_ordering(ordering);

        // See comment from `from`.
        let old = self.inner.load(ordering) as *const _;

        // Safety we know:
        //  * We know the raw pointer was created previously by `C::into_raw` with the same type.
        //  * The strong count will be at least one if there's no multiple drops,
        //    because we converted one of the `C`s to this type.
        C::from_raw(old)
    }

    /// Stores a new `C`. This overwrites the old value, possibly leading to a memory leak if
    /// used without care. The caller should guarantee the use of this is safe.
    pub unsafe fn store(&self, c: C, ordering: Ordering) {
        Self::assert_ordering(ordering);

        // See comment from `from`.
        self.inner.store(C::into_raw(c) as *mut _, ordering);
    }
}

impl<T> IntoRawPtr<T> for Arc<T> {
    fn into_raw(self) -> *const T {
        Arc::into_raw(self)
    }
}

impl<T> FromRawPtr<T> for Arc<T> {
    unsafe fn from_raw(t: *const T) -> Self {
        Arc::from_raw(t)
    }
}

impl<T> IntoRawPtr<T> for Box<T> {
    fn into_raw(self) -> *const T {
        Box::into_raw(self)
    }
}

impl<T> FromRawPtr<T> for Box<T> {
    unsafe fn from_raw(t: *const T) -> Self {
        Box::from_raw(t as *mut _)
    }
}

impl<T> IntoRawPtr<T> for Option<Box<T>> {
    fn into_raw(self) -> *const T {
        match self {
            None => null(),
            Some(b) => Box::into_raw(b),
        }
    }
}

impl<T> FromRawPtr<T> for Option<Box<T>> {
    unsafe fn from_raw(t: *const T) -> Self {
        if t.is_null() {
            None
        } else {
            Some(Box::from_raw(t as *mut _))
        }
    }
}
