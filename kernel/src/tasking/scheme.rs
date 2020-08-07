use crate::sync::spinlock::RwLock;
use crate::tasking::file::{FileDescriptor, FileHandle};
use crate::tasking::scheduler::with_current_thread;
use crate::tasking::thread::ThreadId;
use alloc::collections::VecDeque;
use alloc::sync::Weak;

/// Type of file handle.
enum Handle {
    /// A handle to the scheme itself.
    Own,
    /// A handle to a file in the scheme.
    /// This should be handled by the service.
    Inner,
}

enum CommandData {
    Open,
}

struct Command {
    sender: ThreadId,
    data: CommandData,
}

pub type SchemePtr = Weak<RwLock<Scheme>>;

pub struct Scheme {
    /// Weak pointer to ourself.
    /// Needed to be able to easily create file descriptors.
    pub(crate) ptr: SchemePtr,
    /// Command queue.
    command_queue: VecDeque<Command>,
}

impl Scheme {
    /// Creates a new scheme.
    pub fn new() -> Self {
        Self {
            ptr: Weak::new(),
            command_queue: VecDeque::new(),
        }
    }

    /// Sets the internal pointer.
    pub fn set_ptr(&mut self, ptr: SchemePtr) {
        assert!(self.ptr.upgrade().is_none());
        self.ptr = ptr;
    }

    /// Open a file handle to the scheme itself.
    pub fn open_self(&self) -> FileDescriptor {
        FileDescriptor::from(&self, FileHandle::Own)
    }

    pub fn open(&mut self) {
        let sender = with_current_thread(|thread| thread.id());
        self.command_queue.push_back(Command {
            sender,
            data: CommandData::Open,
        });

        // TODO: wakeup receiver
        // TODO: if this gets pre-empted between "wakeup receiver" and "block this" we will be stuck locked
        //       because the wakeup signal will be sent before this thread gets blocked
        //scheduler::thread_block();

        // TODO
    }
}

impl Default for Scheme {
    fn default() -> Self {
        Self::new()
    }
}
