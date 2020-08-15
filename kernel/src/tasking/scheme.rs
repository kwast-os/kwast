use crate::arch::{preempt_disable, preempt_enable};
use crate::sync::thread_block_guard::ThreadBlockGuard;
use crate::sync::wait_queue::WaitQueue;
use crate::tasking::file::{FileDescriptor, FileHandle};
use crate::tasking::scheduler::{self, with_current_thread};
use crate::tasking::thread::Thread;
use alloc::sync::{Arc, Weak};
use bitflags::_core::sync::atomic::AtomicU64;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Reply data.
/// We only wait at most for one reply. The reply data is very simple, it's just a status + data pair.
/// In the case we have a non-blocking send, we don't have reply data.
pub struct ReplyData {
    status: usize, //  TODO: other type?
    value: u64,
}

/// Reply data inside the Tcb.
pub struct ReplyDataTcb {
    status: AtomicUsize,
    value: AtomicU64,
}

pub enum CommandData {
    Open,
}

struct Command {
    payload: CommandData,
    thread: Arc<Thread>,
    blocking: bool,
}

pub type SchemePtr = Weak<Scheme>;

pub struct Scheme {
    /// Command queue.
    command_queue: WaitQueue<Command>,
}

impl ReplyData {
    /// Creates `ReplyData` from `ReplyDataTcb`.
    pub fn from(reply_data_tcb: &ReplyDataTcb) -> Self {
        let status = reply_data_tcb.status.load(Ordering::Acquire);
        let value = reply_data_tcb.value.load(Ordering::Relaxed);
        Self { status, value }
    }
}

impl ReplyDataTcb {
    /// Creates a new `ReplyDataTcb`.
    pub fn new() -> Self {
        Self {
            status: AtomicUsize::new(usize::MAX),
            value: AtomicU64::new(0),
        }
    }

    /// Stores a new reply.
    pub fn store(&self, data: ReplyData) {
        self.value.store(data.value, Ordering::Relaxed);
        self.status.store(data.status, Ordering::Release);
    }
}

impl Scheme {
    /// Creates a new scheme.
    pub fn new() -> Self {
        Self {
            command_queue: WaitQueue::new(),
        }
    }

    /// Sends a blocking ipc message to the scheme.
    pub fn send_blocking(&self, payload: CommandData) -> ReplyData {
        with_current_thread(|t| {
            // Blocks the thread, sends the command and notifies the receiving thread.
            {
                preempt_disable();
                let _block_guard = ThreadBlockGuard::activate();
                self.command_queue.push_back(Command {
                    payload,
                    thread: t.clone(),
                    blocking: true,
                });
                preempt_enable();
            }

            // Response to sender comes here.
            ReplyData::from(&t.reply)
        })
    }

    /// Opens a file inside the scheme.
    pub(crate) fn open(&self) -> Result<FileHandle, usize> {
        let response = self.send_blocking(CommandData::Open);
        if response.status == 0 {
            Ok(FileHandle::Inner(response.value))
        } else {
            Err(response.status)
        }
    }

    fn command_receive(&self) -> Command {
        self.command_queue.pop_front()
    }

    pub fn test(&self, data2: i32) {
        let cmd = self.command_receive();
        // TODO: set reply data
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
