use crate::arch::address::VirtAddr;
use crate::tasking::scheduler::{self, with_core_scheduler, SwitchReason};
use crate::wasm::main::{WASM_CALL_CONV, WASM_VMCTX_TYPE};
use crate::wasm::vmctx::VmContext;
use core::cell::Cell;
use core::marker::PhantomData;
use core::mem::{align_of, size_of};
use core::slice;
use cranelift_codegen::ir::{types, AbiParam, ArgumentPurpose, Signature};
use hashbrown::HashMap;
use lazy_static::lazy_static;

// See https://github.com/WebAssembly/WASI/blob/master/phases/snapshot/docs.md
#[repr(u16)]
#[allow(dead_code)]
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
struct WasmPtr<T> {
    offset: u32,
    _phantom: PhantomData<T>,
}

impl<T> WasmPtr<T> {
    /// Internal helper function to get a real pointer or an error from a WasmPtr.
    fn get_ptr_and_verify(&self, ctx: &VmContext, size: usize) -> WasmResult<*const u8> {
        let alignment = align_of::<T>() as u32;
        if self.offset % alignment != 0
            || self.offset as usize + size
                > with_core_scheduler(|s| s.get_current_thread().heap_size())
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
    pub fn slice<'s>(&self, ctx: &VmContext, len: u32) -> WasmResult<&'s [Cell<T>]> {
        let len = len as usize;

        // Safety: pointer is correctly aligned and points to real data.
        self.get_ptr_and_verify(
            ctx,
            (size_of::<T>() + (size_of::<T>() % align_of::<T>())) * len,
        )
        .map(|p| unsafe { slice::from_raw_parts(p as *const Cell<T>, len) })
    }
}

/// Size type.
type Size = u32;

/// File descriptor.
type Fd = u32;

/// Exit code for process.
type ExitCode = u32;

type WasmResult<T> = Result<T, Errno>;
type WasmStatus = WasmResult<()>;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct CioVec {
    pub buf: WasmPtr<u8>,
    pub buf_len: u32,
}

abi_functions! {
    environ_sizes_get: (environc: WasmPtr<Size>, environ_buf_size: WasmPtr<Size>) -> Errno,
    environ_get: (environ: WasmPtr<WasmPtr<u8>>, environ_buf: WasmPtr<u8>) -> Errno,
    fd_write: (fd: Fd, iovs: WasmPtr<CioVec>, iovs_len: u32, nwritten: WasmPtr<u32>) -> Errno,
    proc_exit: (exit_code: ExitCode) -> (),
}

impl AbiFunctions for VmContext {
    fn environ_sizes_get(
        &self,
        environc: WasmPtr<Size>,
        environ_buf_size: WasmPtr<Size>,
    ) -> WasmStatus {
        println!("environ_sizes_get");
        environc.cell(self)?.set(0);
        environ_buf_size.cell(self)?.set(0);
        Ok(())
    }

    fn environ_get(&self, _environ: WasmPtr<WasmPtr<u8>>, _environ_buf: WasmPtr<u8>) -> WasmStatus {
        // TODO
        println!("environ_get");
        Ok(())
    }

    fn fd_write(
        &self,
        fd: Fd,
        iovs: WasmPtr<CioVec>,
        iovs_len: u32,
        nwritten: WasmPtr<u32>,
    ) -> WasmStatus {
        println!("fd_write {} iovs_len={}", fd, iovs_len);

        let iovs = iovs.slice(self, iovs_len)?;

        // TODO: overflow?
        let mut written = 0;

        for iov in iovs {
            let iov = iov.get();

            let buf = iov.buf.slice(self, iov.buf_len)?;

            // TODO: just prints to stdout for now
            print!("Got: ");
            for b in buf {
                print!("{}", b.get() as char);
            }
            println!();

            written += iov.buf_len;
        }

        nwritten.cell(&self)?.set(written);

        Ok(())
    }

    fn proc_exit(&self, exit_code: ExitCode) {
        // TODO: exit code
        println!("proc_exit: exit code {}", exit_code);
        scheduler::switch_to_next(SwitchReason::Exit);
        unreachable!("thread exit")
    }
}

/// Gets the address for a wasi syscall and validate signature.
pub fn get_address_for_wasi_and_validate_sig(name: &str, sig: &Signature) -> Option<VirtAddr> {
    let (addr, reference_sig) = ABI_MAP.get(name)?;

    println!("{:?}", reference_sig);
    println!("{:?}", sig);

    if reference_sig != sig {
        None
    } else {
        Some(*addr)
    }
}
