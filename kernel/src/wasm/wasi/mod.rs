//! Wasi implementation
//! See https://github.com/WebAssembly/WASI/blob/master/phases/snapshot/docs.md

#![allow(clippy::too_many_arguments)]
#![allow(clippy::identity_op)]

mod definitions;

pub use definitions::*;

use crate::arch::address::VirtAddr;
use crate::tasking::file::{FileDescriptor, FileHandle, FileIdx};
use crate::tasking::scheduler::{self, with_current_thread};
use crate::tasking::scheme::Scheme;
use crate::tasking::scheme_container::schemes;
use crate::wasm::main::{WASM_CALL_CONV, WASM_VMCTX_TYPE};
use crate::wasm::vmctx::VmContext;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::convert::{TryFrom, TryInto};
use core::slice;
use cranelift_codegen::ir::{types, AbiParam, ArgumentPurpose, Signature};
use lazy_static::lazy_static;

abi_functions! {
    environ_sizes_get: (environc: WasmPtr<Size>, environ_buf_size: WasmPtr<Size>) -> Errno,
    environ_get: (environ: WasmPtr<WasmPtr<u8>>, environ_buf: WasmPtr<u8>) -> Errno,
    fd_close: (fd: Fd) -> Errno,
    fd_read: (fd: Fd, iovs: WasmPtr<CioVec>, iovs_len: Size, nread: WasmPtr<u32>) -> Errno,
    fd_write: (fd: Fd, iovs: WasmPtr<CioVec>, iovs_len: Size, nwritten: WasmPtr<u32>) -> Errno,
    fd_prestat_get: (fd: Fd, prestat: WasmPtr<PreStat>) -> Errno,
    fd_prestat_dir_name: (fd: Fd, path: WasmPtr<u8>, path_len: Size) -> Errno,
    path_open: (dir_fd: Fd, dir_flags: LookupFlags, path: WasmPtr<u8>, path_len: Size, o_flags: OFlags, fs_rights_base: Rights, fs_rights_inheriting: Rights, fd_flags: FdFlags, fd: WasmPtr<Fd>) -> Errno,
    proc_exit: (exit_code: ExitCode) -> (),
}

// TODO: capabilities
impl AbiFunctions for VmContext {
    fn environ_sizes_get(
        &self,
        environc: WasmPtr<Size>,
        environ_buf_size: WasmPtr<Size>,
    ) -> WasmStatus {
        environc.cell(self)?.set(1);
        // This is the sum of the string lengths in bytes (including \0 terminators)
        let abcdefg = "RUST_BACKTRACE=1";
        environ_buf_size
            .cell(self)?
            .set(1 + abcdefg.bytes().len() as u32 /* TODO: make safe */);
        //environc.cell(self)?.set(0);
        //environ_buf_size.cell(self)?.set(0);
        Ok(())
    }

    fn environ_get(&self, environ: WasmPtr<WasmPtr<u8>>, environ_buf: WasmPtr<u8>) -> WasmStatus {
        // The bytes should be all after each other consecutively in `environ_buf`.
        let abcdefg = "RUST_BACKTRACE=1";
        let slice = environ_buf.slice(
            &self,
            (1 + abcdefg.bytes().len()) as u32, /* TODO: make safe */
        )?;
        for (byte, cell) in abcdefg.bytes().zip(slice.iter()) {
            cell.set(byte);
        }
        slice[slice.len() - 1].set(0);

        // Write pointers to the environment variables in the buffer.
        let slice = environ.slice(&self, 1)?;
        slice[0].set(WasmPtr::from(environ_buf.offset()));

        Ok(())
    }

    fn fd_close(&self, fd: Fd) -> WasmStatus {
        println!("fd_close: {}", fd);
        Ok(())
    }

    fn fd_read(
        &self,
        fd: Fd,
        iovs: WasmPtr<CioVec>,
        iovs_len: u32,
        nread: WasmPtr<u32>,
    ) -> WasmStatus {
        self.with_fd_handle(fd, |scheme, handle| {
            let mut read = 0usize;
            let iovs = iovs.slice(self, iovs_len)?;
            for iov in iovs {
                let iov = iov.get();
                let buf = iov.buf.slice(self, iov.buf_len)?;

                // TODO: safety
                let buf =
                    unsafe { slice::from_raw_parts_mut(buf as *const _ as *mut u8, buf.len()) };
                let read_now = scheme.read(handle, buf)?;
                read = read.saturating_add(read_now);
            }

            nread.cell(self)?.set(read.try_into().unwrap_or(u32::MAX));

            Ok(())
        })
    }

    fn fd_write(
        &self,
        fd: Fd,
        iovs: WasmPtr<CioVec>,
        iovs_len: u32,
        nwritten: WasmPtr<u32>,
    ) -> WasmStatus {
        //println!("fd_write {} iovs_len={}", fd, iovs_len);

        // TODO: debug
        if fd < 3 {
            let iovs = iovs.slice(self, iovs_len)?;

            // TODO: overflow?
            let mut written = 0;

            for iov in iovs {
                let iov = iov.get();

                let buf = iov.buf.slice(self, iov.buf_len)?;

                // TODO: just prints to stdout for now
                for b in buf {
                    print!("{}", b.get() as char);
                }

                written += iov.buf_len;
            }

            nwritten.cell(&self)?.set(written);
            return Ok(());
        }

        self.with_fd_handle(fd, |scheme, handle| {
            let mut written = 0usize;
            let iovs = iovs.slice(self, iovs_len)?;
            for iov in iovs {
                let iov = iov.get();
                let buf = iov.buf.slice(self, iov.buf_len)?;

                // TODO: safety
                let buf = unsafe { slice::from_raw_parts(buf as *const _ as *const u8, buf.len()) };
                let written_now = scheme.write(handle, buf)?;
                written = written.saturating_add(written_now);
            }

            nwritten
                .cell(self)?
                .set(written.try_into().unwrap_or(u32::MAX));

            Ok(())
        })
    }

    fn fd_prestat_get(&self, fd: Fd, prestat: WasmPtr<PreStat>) -> WasmStatus {
        self.with_fd(fd, |fd| {
            // TODO: check if it's a directory, if it's not: return ENOTDIR
            let pre_open_path = fd.pre_open_path().ok_or(Errno::NotSup)?;
            if let Ok(pr_name_len) = u32::try_from(pre_open_path.len() + 1) {
                prestat.cell(self)?.set(PreStat {
                    tag: 0,
                    inner: PreStatInner {
                        dir: PreStatDir { pr_name_len },
                    },
                });
                //println!("fd_prestat_get: write {}", pr_name_len);
                Ok(())
            } else {
                Err(Errno::NameTooLong)
            }
        })
    }

    fn fd_prestat_dir_name(&self, fd: Fd, path: WasmPtr<u8>, path_len: Size) -> WasmStatus {
        self.with_fd(fd, |fd| {
            // TODO: check if it's a directory, if it's not: return ENOTDIR
            let pre_open_path = fd.pre_open_path().ok_or(Errno::NotSup)?;
            if pre_open_path.len() + 1 > path_len as usize {
                Err(Errno::NameTooLong)
            } else {
                //println!("fd_prestat_dir_name: {:?}", pre_open_path);
                path.write_from_slice_with_null(self, path_len, pre_open_path)
            }
        })
    }

    fn path_open(
        &self,
        dir_fd: Fd,
        dir_flags: LookupFlags,
        path: WasmPtr<u8>,
        path_len: Size,
        o_flags: OFlags,
        fs_rights_base: Rights,
        fs_rights_inheriting: Rights,
        fd_flags: FdFlags,
        fd: WasmPtr<Fd>,
    ) -> WasmStatus {
        // TODO: handle the flags and rights
        println!("path_open: {} {}", dir_fd, path.str(self, path_len)?);

        /*self.with_fd(dir_fd, |dir_fd| {

            // TODO
            fd.cell(&self)?.set(3); // TODO: hack
            Ok(())
        })*/
        let idx = with_current_thread(|t| {
            t.file_descriptor_table().insert_lowest({
                schemes()
                    .read()
                    .open_self(Box::new([]))
                    .expect("self scheme")
            })
        })
        .unwrap(); // TODO

        fd.cell(self)?.set(idx as u32);

        Ok(())
    }

    fn proc_exit(&self, exit_code: ExitCode) {
        scheduler::thread_exit(exit_code);
    }
}

impl VmContext {
    /// Execute with fd handle context.
    fn with_fd_handle<F, T>(&self, fd: Fd, f: F) -> WasmResult<T>
    where
        F: FnOnce(Arc<Scheme>, FileHandle) -> WasmResult<T>,
    {
        with_current_thread(|thread| {
            let tbl = thread.file_descriptor_table();
            let fd = tbl.get(fd as FileIdx).ok_or(Errno::BadF)?;
            let (scheme, handle) = fd.scheme_and_handle()?;
            drop(tbl);
            f(scheme, handle)
        })
    }

    /// Execute with full fd context.
    fn with_fd<F, T>(&self, fd: Fd, f: F) -> WasmResult<T>
    where
        F: FnOnce(&FileDescriptor) -> WasmResult<T>,
    {
        with_current_thread(|thread| {
            let tbl = thread.file_descriptor_table();
            f(tbl.get(fd as FileIdx).ok_or(Errno::BadF)?)
        })
    }
}

/// Gets the address for a wasi syscall and validate signature.
pub fn get_address_for_wasi_and_validate_sig(name: &str, sig: &Signature) -> Option<VirtAddr> {
    let (addr, reference_sig) = ABI_MAP.get(name)?;

    if reference_sig != sig {
        None
    } else {
        Some(*addr)
    }
}
