use crate::arch::address::VirtAddr;
use crate::tasking::scheduler;
use crate::tasking::scheduler::SwitchReason;
use crate::wasm::vmctx::VmContext;
use core::cell::Cell;
use core::marker::PhantomData;
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
    // TODO: more
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
struct WasmPtr<T> {
    offset: u32,
    _phantom: PhantomData<T>,
}

impl<T> WasmPtr<T> {
    // TODO: docs
    pub fn deref<'c>(&self, ctx: &VmContext) -> WasmResult<&'c Cell<T>> {
        // TODO: bounds check
        // TODO: alignment check?

        unsafe {
            let addr = ctx.heap_ptr.as_const::<u8>().add(self.offset as usize);
            Ok(&*(addr as *const Cell<T>))
        }
    }
}

type Size = u32;

type Fd = u32;

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
    proc_exit: (exit_code: ExitCode) -> Errno,
}

impl AbiFunctions for VmContext {
    fn environ_sizes_get(
        &self,
        environc: WasmPtr<Size>,
        environ_buf_size: WasmPtr<Size>,
    ) -> WasmStatus {
        println!("environ_sizes_get");
        environc.deref(self)?.set(0);
        environ_buf_size.deref(self)?.set(0);
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

        // TODO: it's actually an array
        let iovs = iovs.deref(self)?;
        println!("{:?} {}", iovs.get().buf, iovs.get().buf_len);

        // HACK HACK HACK
        let mut buf = iovs.get().buf;
        let buf_len = iovs.get().buf_len;

        print!("Got: ");
        for _ in 0..buf_len {
            print!("{}", buf.deref(&self)?.get() as char);

            // HACK HACK HACK
            buf.offset += 1;
        }
        println!();

        nwritten.deref(&self)?.set(buf_len);

        Ok(())
    }

    fn proc_exit(&self, exit_code: ExitCode) -> WasmStatus {
        // TODO: exit code
        println!("exit code {}", exit_code);
        scheduler::switch_to_next(SwitchReason::Exit);
        unreachable!("thread exit")
    }
}

/// Gets the address for a wasi syscall.
pub fn get_address_for_wasi(name: &str) -> Option<VirtAddr> {
    ABI_MAP.get(name).map(|e| VirtAddr::new(*e))
}
