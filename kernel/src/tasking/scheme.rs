use crate::arch::{preempt_disable, preempt_enable};
use crate::mm::tcb_alloc::with_thread;
use crate::sync::thread_block_guard::ThreadBlockGuard;
use crate::sync::wait_queue::WaitQueue;
use crate::tasking::file::{FileHandle, InnerFileHandle};
use crate::tasking::scheduler::{self, with_current_thread};
use crate::tasking::scheme_container::SchemeId;
use crate::tasking::thread::ThreadId;
use crate::wasm::wasi::Errno;
use alloc::sync::Weak;
use atomic::Atomic;
use core::mem::size_of;
use core::slice;
use core::sync::atomic::{AtomicU64, Ordering};

/// Reply payload.
/// We only wait at most for one reply. The reply data is very simple, it's just a status + data pair.
/// In the case we have a non-blocking send, we don't have reply data.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct ReplyPayload {
    status: Errno,
    value: u64,
}

/// Reply payload inside the Tcb.
pub struct ReplyPayloadTcb {
    status: Atomic<Errno>,
    value: AtomicU64,
}

/// Reply from userspace.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Reply {
    to: ThreadId,
    payload: ReplyPayload,
}

#[repr(C)]
pub enum CommandData {
    Open(i32), // TODO: test
    Read(InnerFileHandle),
}

#[repr(C)]
pub struct Command {
    thread_id: ThreadId,
    payload: CommandData,
}

pub type SchemePtr = Weak<Scheme>;

// TODO: capability instead of thread sender
pub struct Scheme {
    /// Identifier: needed for `blocked_on` in tcb.
    id: SchemeId,
    /// Command queue.
    command_queue: WaitQueue<Command>,
}

impl ReplyPayload {
    /// Creates `ReplyData` from `ReplyDataTcb`.
    pub fn from(reply_data_tcb: &ReplyPayloadTcb) -> Self {
        let status = reply_data_tcb.status.load(Ordering::Acquire);
        let value = reply_data_tcb.value.load(Ordering::Relaxed);
        Self { status, value }
    }
}

impl ReplyPayloadTcb {
    /// Creates a new `ReplyDataTcb`.
    pub fn new() -> Self {
        Self {
            status: Atomic::new(Errno::Success),
            value: AtomicU64::new(0),
        }
    }

    /// Stores a new reply.
    pub fn store(&self, data: ReplyPayload) {
        self.value.store(data.value, Ordering::Relaxed);
        self.status.store(data.status, Ordering::Release);
    }
}

impl Scheme {
    /// Creates a new scheme.
    pub(crate) fn new(id: SchemeId) -> Self {
        Self {
            id,
            command_queue: WaitQueue::new(),
        }
    }

    /// Sends a blocking IPC message to the scheme.
    pub fn send_command_blocking(&self, payload: CommandData) -> ReplyPayload {
        with_current_thread(|t| {
            // Blocks the thread, sends the command and notifies the receiving thread.
            {
                preempt_disable();
                let _block_guard = ThreadBlockGuard::activate();
                //self.a.lock().insert(t.id, t.clone());
                t.set_ipc_blocked_on(self.id);
                self.command_queue.push_back(Command {
                    payload,
                    thread_id: t.id,
                });
                preempt_enable();
            }

            t.set_ipc_blocked_on(SchemeId::sentinel());

            // Response to sender comes here.
            ReplyPayload::from(&t.reply)
        })
    }

    pub fn send_replies(&self, buffer: &[u8]) -> Result<usize, Errno> {
        // TODO: document
        let buffer = unsafe {
            slice::from_raw_parts(
                buffer as *const _ as *const Reply,
                buffer.len() / size_of::<Reply>(),
            )
        };

        for reply in buffer {
            self.send_reply(*reply);
        }

        Ok(buffer.len() * size_of::<Reply>())
    }

    /// Opens a file inside the scheme.
    pub(crate) fn open(&self, lol: i32) -> Result<FileHandle, Errno> {
        let response = self.send_command_blocking(CommandData::Open(lol));
        match response.status {
            Errno::Success => Ok(FileHandle::Inner(InnerFileHandle(response.value))),
            e => Err(e),
        }
    }

    pub fn write(&self, handle: FileHandle, buffer: &[u8]) -> Result<usize, Errno> {
        // TODO: needs grants
        match handle {
            FileHandle::Own => self.send_replies(buffer),
            FileHandle::Inner(handle) => self.regular_write(handle, buffer),
        }
    }

    pub fn read(&self, handle: FileHandle, buffer: &mut [u8]) -> Result<usize, Errno> {
        match handle {
            FileHandle::Own => self.receive_commands_blocking(buffer),
            FileHandle::Inner(handle) => self.regular_read(handle, buffer),
        }
    }

    pub fn send_reply(&self, reply: Reply) {
        let success = with_thread(reply.to, |receiver| {
            if receiver.ipc_blocked_on() != self.id {
                false
            } else {
                receiver.reply.store(reply.payload);
                true
            }
        });
        /*let success = if let Some(receiver) = self.a.lock().remove(&reply.to) {
            receiver.reply.store(reply.payload);
            true
        } else {
            false
        };*/

        // This needs to be outside the lock.
        if success {
            scheduler::wakeup_and_yield(reply.to);
        }
    }

    pub fn receive_commands_blocking(&self, buffer: &mut [u8]) -> Result<usize, Errno> {
        // TODO: document
        let buffer = unsafe {
            slice::from_raw_parts_mut(
                buffer as *mut _ as *mut Command,
                buffer.len() / size_of::<Command>(),
            )
        };

        let x = self.command_queue.pop_front_many(buffer);
        Ok(x * size_of::<Command>())
        // TODO: map memory if required?
    }

    pub fn regular_read(&self, handle: InnerFileHandle, buffer: &mut [u8]) -> Result<usize, Errno> {
        // TODO: share memory or smth
        let reply = self.send_command_blocking(CommandData::Read(handle));
        // TODO: ??

        Ok(reply.value as usize)
    }

    pub fn regular_write(&self, handle: InnerFileHandle, buffer: &[u8]) -> Result<usize, Errno> {
        // TODO
        Ok(0)
    }
}
