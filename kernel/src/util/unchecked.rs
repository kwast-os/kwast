pub trait UncheckedUnwrap<T> {
    /// Unwraps a type without the cost of the branch: no safety check will be performed.
    /// If you're in debug mode, will assert.
    unsafe fn unchecked_unwrap(self) -> T;
}

impl<T> UncheckedUnwrap<T> for Option<T> {
    unsafe fn unchecked_unwrap(self) -> T {
        debug_assert!(self.is_some());

        if let Some(inner) = self {
            inner
        } else {
            core::hint::unreachable_unchecked();
        }
    }
}
