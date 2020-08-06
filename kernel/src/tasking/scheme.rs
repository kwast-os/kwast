use crate::sync::spinlock::RwLock;
use crate::tasking::file::{FileDescriptor, FileHandle};
use alloc::sync::Weak;

enum Handle {
    Own,
    Scheme,
}

pub type SchemePtr = Weak<RwLock<Scheme>>;

#[derive(Debug)]
pub struct Scheme {
    ptr: SchemePtr,
}

impl Scheme {
    /// Creates a new scheme.
    pub fn new() -> Self {
        Self { ptr: Weak::new() }
    }

    /// Sets the internal pointer.
    pub fn set_ptr(&mut self, ptr: SchemePtr) {
        assert!(self.ptr.upgrade().is_none());
        self.ptr = ptr;
    }

    /// Open a file handle to the scheme itself.
    pub fn open_self(&self) -> FileDescriptor {
        FileDescriptor::from(self.ptr.clone(), FileHandle::Own)
    }
}

impl Default for Scheme {
    fn default() -> Self {
        Self::new()
    }
}
