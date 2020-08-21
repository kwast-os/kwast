use crate::tasking::scheduler::with_current_thread;
use crate::wasm::vmctx::VmContext;
use bitflags::bitflags;
use core::cell::Cell;
use core::marker::PhantomData;
use core::mem::{align_of, size_of};
use core::{iter, slice};

#[repr(u32)] // 32-bit for Cranelift
#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub enum Errno {
    /// No error occurred.
    Success,
    /// Argument list too long.
    ArgListTooBig,
    /// Permission denied.
    Access,
    /// Address in use.
    AddrInUse,
    /// Address not available.
    AddrNotAvail,
    /// Address family not supported.
    AfNoSupport,
    /// Resource unavailable, or operation would block.
    Again,
    /// Connection already in progress.
    Already,
    /// Bad file descriptor.
    BadF,
    /// Bad message.
    BadMsg,
    /// Device or resource busy.
    Busy,
    /// Operation canceled.
    Canceled,
    /// No child process.
    Child,
    /// Connection aborted.
    ConnAborted,
    /// Connection refused.
    ConnRefused,
    /// Connection reset.
    ConnReset,
    /// Resource deadlock would occur.
    DeadLk,
    /// Destination address required.
    DestAddrReq,
    /// Mathematics argument out of domain of function.
    Dom,
    /// Reserved.
    Dquot,
    /// File exists.
    Exist,
    /// Bad address.
    Fault,
    /// File too large.
    FBig,
    /// Host is unreachable.
    HostUnreach,
    /// Identifier removed.
    Idrm,
    /// Illegal byte sequence.
    Ilseq,
    /// Operation in progress.
    Inprogress,
    /// Interrupted function.
    Intr,
    /// Invalid argument.
    Inval,
    /// I/O error.
    Io,
    /// Socket is connected.
    IsConn,
    /// Is a directory.
    Isdir,
    /// Too many levels of symbolic links.
    Loop,
    /// File descriptor value too large.
    MFile,
    /// Too many links.
    Mlink,
    /// Message too large.
    MsgSize,
    /// Reserved.
    Multihop,
    /// Filename too long.
    NameTooLong,
    /// Network is down.
    NetDown,
    /// Connection aborted by network.
    NetReset,
    /// Network unreachable.
    NetUnreach,
    /// Too many files open in system.
    NFile,
    /// No buffer space available.
    NoBufs,
    /// No such device.
    NoDev,
    /// No such file or directory.
    NoEnt,
    /// Executable file format error.
    NoExec,
    /// No locks available.
    NoLck,
    /// Reserved.
    NoLink,
    /// Not enough space.
    NoMem,
    /// No message of the desired type.
    NoMsg,
    /// Protocol not available.
    NoProtoopt,
    /// No space left on device.
    NoSpc,
    /// Function not supported.
    NoSys,
    /// The socket is not connected.
    NotConn,
    /// Not a directory or a symbolic link to a directory.
    NotDir,
    /// Directory not empty.
    NotEmpty,
    /// State not recoverable.
    NotRecoverable,
    /// Not a socket.
    NotSock,
    /// Not supported, or operation not supported on socket.
    NotSup,
    /// Inappropriate I/O control operation.
    NoTty,
    /// No such device or address.
    Nxio,
    /// Value too large to be stored in data type.
    Overflow,
    /// Previous owner died.
    Ownerdead,
    /// Operation not permitted.
    Perm,
    /// Broken pipe.
    Pipe,
    /// Protocol error.
    Proto,
    /// Protocol not supported.
    Protonosupport,
    /// Protocol wrong type for socket.
    Prototype,
    /// Result too large.
    Range,
    /// Read-only file system.
    Rofs,
    /// Invalid seek.
    Spipe,
    /// No such process.
    Srch,
    /// Reserved.
    Stale,
    /// Connection timed out.
    TimedOut,
    /// Text file busy.
    Txtbsy,
    /// Cross-device link.
    Xdev,
    /// Extension: Capabilities insufficient.
    NotCapable,
}

/// WebAssembly pointer type to use in ABI functions.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct WasmPtr<T> {
    offset: u32,
    _phantom: PhantomData<T>,
}

impl<T> WasmPtr<T> {
    /// Creates a WasmPtr from an offset.
    pub fn from(offset: u32) -> Self {
        Self {
            offset,
            _phantom: PhantomData,
        }
    }

    /// Gets the offset from the base memory address.
    #[inline]
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Internal helper function to get a real pointer or an error from a WasmPtr.
    fn get_ptr_and_verify(&self, ctx: &VmContext, size: usize) -> WasmResult<*const u8> {
        let alignment = align_of::<T>() as u32;
        if self.offset % alignment != 0
            || self.offset as usize + size > with_current_thread(|thread| thread.heap_size())
        {
            Err(Errno::Fault)
        } else {
            // Safety: pointer is correctly aligned and points to real data.
            unsafe { Ok(ctx.heap_ptr.as_const::<u8>().add(self.offset as usize)) }
        }
    }

    /// Gets a cell from a Wasm pointer, does checks for alignment and bounds.
    /// Returns Ok(Cell) on success and Err(Errno) on fail.
    pub fn cell<'c>(&self, ctx: &VmContext) -> WasmResult<&'c Cell<T>> {
        // Safety: pointer is correctly aligned and points to real data.
        self.get_ptr_and_verify(ctx, size_of::<T>())
            .map(|p| unsafe { &*(p as *const Cell<T>) })
    }

    /// Gets a slice of cells from a Wasm pointer, does checks for alignment and bounds.
    /// Returns Ok(slice) on success and Err(Errno) on fail.
    pub fn slice<'s>(&self, ctx: &VmContext, len: Size) -> WasmResult<&'s [Cell<T>]> {
        let len = len as usize;

        // Safety: pointer is correctly aligned and points to real data.
        self.get_ptr_and_verify(
            ctx,
            (size_of::<T>() + (size_of::<T>() % align_of::<T>())) * len,
        )
        .map(|p| unsafe { slice::from_raw_parts(p as *const Cell<T>, len) })
    }

    /// Gets a string from a Wasm pointer, does checks for valid UTF-8 string.
    /// Returns Ok(str) on success and Err(Errno) on fail.
    pub fn str<'s>(&self, ctx: &VmContext, len: Size) -> WasmResult<&'s str> {
        let len = len as usize;
        self.get_ptr_and_verify(ctx, len)
            .map(|p| unsafe { slice::from_raw_parts(p, len) })
            .and_then(|p| core::str::from_utf8(p).map_err(|_| Errno::Inval))
    }
}

impl WasmPtr<u8> {
    /// Write from source slice and add a null byte.
    pub fn write_from_slice_with_null(
        &self,
        ctx: &VmContext,
        len: Size,
        src: &[u8],
    ) -> WasmResult<()> {
        let slice = self.slice(ctx, len)?;
        for (dst, src) in slice.iter().zip(src.iter().chain(iter::once(&0))) {
            dst.set(*src);
        }
        Ok(())
    }
}

/// Size type.
pub type Size = u32;

/// File descriptor.
pub type Fd = u32;

/// Exit code for process.
pub type ExitCode = u32;

pub type WasmResult<T> = Result<T, Errno>;
pub type WasmStatus = WasmResult<()>;

bitflags! {
    #[repr(C)]
    pub struct LookupFlags: u32 {
        const SYMLINK_FOLLOW = 1 << 0;
    }
}

bitflags! {
    #[repr(C)]
    pub struct OFlags: u16 {
        const CREAT = 1 << 0;
        const DIRECTORY = 1 << 1;
        const EXCL = 1 << 2;
        const TRUNC = 1 << 3;
    }
}

bitflags! {
    #[repr(C)]
    pub struct FdFlags: u16 {
        const APPEND = 1 << 0;
        const DSYNC = 1 << 1;
        const NONBLOCK = 1 << 2;
        const RSYNC = 1 << 3;
        const SYNC = 1 << 4;
    }
}

bitflags! {
    #[repr(C)]
    pub struct Rights: u64 {
        const FD_DATASYNC = 1 << 0;
        const FD_READ = 1 << 1;
        const FD_SEEK = 1 << 2;
        const FD_FDSTAT_SET_FLAGS = 1 << 3;
        const FD_SYNC = 1 << 4;
        const FD_TELL = 1 << 5;
        const FD_WRITE = 1 << 6;
        const FD_ADVISE = 1 << 7;
        const FD_ALLOCATE = 1 << 8;
        const PATH_CREATE_DIRECTORY = 1 << 9;
        const PATH_CREATE_FILE = 1 << 10;
        const PATH_LINK_SOURCE = 1 << 11;
        const PATH_LINK_TARGET = 1 << 12;
        const PATH_OPEN = 1 << 13;
        const FD_READDIR = 1 << 14;
        const PATH_READLINK = 1 << 15;
        const PATH_RENAME_SOURCE = 1 << 16;
        const PATH_RENAME_TARGET = 1 << 17;
        const PATH_FILESTAT_GET = 1 << 18;
        const PATH_FILESTAT_SET_SIZE = 1 << 19;
        const PATH_FILESTAT_SET_TIMES = 1 << 20;
        const FD_FILESTAT_GET = 1 << 21;
        const FD_FILESTAT_SET_SIZE = 1 << 22;
        const FD_FILESTAT_SET_TIMES = 1 << 23;
        const PATH_SYMLINK = 1 << 24;
        const PATH_REMOVE_DIRECTORY = 1 << 25;
        const PATH_UNLINK_FILE = 1 << 26;
        const POLL_FD_READWRITE = 1 << 27;
        const SOCK_SHUTDOWN = 1 << 28;
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CioVec {
    pub buf: WasmPtr<u8>,
    pub buf_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PreStatDir {
    pub pr_name_len: Size,
}

#[repr(C)]
pub union PreStatInner {
    pub dir: PreStatDir,
}

#[repr(C)]
pub struct PreStat {
    pub tag: u8,
    pub inner: PreStatInner,
}
