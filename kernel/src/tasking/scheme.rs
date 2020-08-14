use crate::arch::{preempt_disable, preempt_enable};
use crate::sync::thread_block_guard::ThreadBlockGuard;
use crate::sync::wait_queue::WaitQueue;
use crate::tasking::file::{FileDescriptor, FileHandle};
use crate::tasking::scheduler::{self, with_current_thread};
use crate::tasking::thread::Thread;
use alloc::sync::{Arc, Weak};
use core::sync::atomic::Ordering;

/// Reply data inside TCB.
/// We only wait at most for one reply. The reply data is very simple, it's just a status + data pair.
/// In the case we have a non-blocking send, we don't have reply data.
pub type TcbReplyData = u64; // TODO: move me and make better

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
    data: CommandData,
    thread: Arc<Thread>,
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

    pub fn open(&self) -> bool {
        with_current_thread(|t| {
            // Blocks the thread, sends the command and notifies the receiving thread.
            {
                preempt_disable();
                let _block_guard = ThreadBlockGuard::activate();
                self.command_queue.push_back(Command {
                    data: CommandData::Open,
                    thread: t.clone(),
                });
                preempt_enable();
            }

            t.abc.load(Ordering::Acquire) == 1000000
        })
    }

    fn command_receive(&self) -> Command {
        self.command_queue.pop_front()
    }

    pub fn test(&self, data2: i32) {
        let cmd = self.command_receive();
        cmd.thread.abc.store(data2, Ordering::Release);
        let id = cmd.thread.id();
        drop(cmd.thread);
        scheduler::wakeup_and_yield(id);
    }
}

impl Default for Scheme {
    fn default() -> Self {
        Self::new()
    }
}
