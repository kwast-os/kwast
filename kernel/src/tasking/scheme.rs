use crate::arch::{preempt_disable, preempt_enable};
use crate::sync::atomic::AtomicOptionBox;
use crate::sync::thread_block_guard::ThreadBlockGuard;
use crate::sync::wait_queue::WaitQueue;
use crate::tasking::file::{FileDescriptor, FileHandle};
use crate::tasking::scheduler::{self, with_current_thread};
use crate::tasking::thread::ThreadId;
use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};
use core::sync::atomic::Ordering;

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

pub struct Command {
    sender: ThreadId,
    data: CommandData,
    response: Arc<AtomicOptionBox<i32>>,
}

pub type SchemePtr = Weak<Scheme>;

pub struct Scheme {
    /// Weak pointer to ourself.
    /// Needed to be able to easily create file descriptors.
    pub(crate) ptr: SchemePtr,
    /// Command queue.
    command_queue: WaitQueue<Command>,
}

impl Scheme {
    /// Creates a new scheme.
    pub fn new() -> Self {
        Self {
            ptr: Weak::new(),
            command_queue: WaitQueue::new(),
        }
    }

    /// Sets the internal pointer.
    pub fn set_ptr(&self, ptr: SchemePtr) {
        assert!(self.ptr.upgrade().is_none());
        // TODO
        //self.ptr = ptr;
    }

    /// Open a file handle to the scheme itself.
    pub fn open_self(&self) -> FileDescriptor {
        FileDescriptor::from(&self, FileHandle::Own)
    }

    pub fn open(&self) {
        let sender = with_current_thread(|thread| thread.id());
        let response = Arc::new(AtomicOptionBox::from(None));

        // Blocks the thread, sends the command and notifies the receiving thread.
        {
            preempt_disable();
            let _block_guard = ThreadBlockGuard::activate();
            self.command_queue.push_back(Command {
                sender,
                data: CommandData::Open,
                response: response.clone(),
            });
            preempt_enable();
        }

        // We have waken up
        let response = response.swap(None, Ordering::AcqRel);
        println!("response: {:?}", response);

        // TODO
    }

    pub fn command_receive(&self) -> Command {
        self.command_queue.pop_front()
    }

    pub fn test(&self, data: i32) {
        let cmd = self.command_receive();
        let response = Some(Box::new(data));
        cmd.response.swap(response, Ordering::AcqRel);
        scheduler::wakeup_and_yield(cmd.sender);
    }
}

impl Default for Scheme {
    fn default() -> Self {
        Self::new()
    }
}
