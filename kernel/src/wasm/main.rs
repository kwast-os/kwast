//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::{CodegenError, Context};
use cranelift_wasm::translate_module;
use cranelift_wasm::{FuncIndex, FuncTranslator, WasmError};

use crate::arch;
use crate::arch::address::{align_up, VirtAddr};
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::mm::vma_allocator::{MappableVma, Vma};
use crate::tasking::scheduler::{add_and_schedule_thread, with_core_scheduler};
use crate::tasking::thread::Thread;
use crate::wasm::func_env::FuncEnv;
use crate::wasm::module_env::{FunctionBody, FunctionImport, ModuleEnv};
use crate::wasm::reloc_sink::{RelocSink, RelocationTarget};
use crate::wasm::vmctx::{VmContext, VmContextContainer, HEAP_GUARD_SIZE, HEAP_SIZE};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr::write_unaligned;
use cranelift_codegen::binemit::{NullStackmapSink, NullTrapSink, Reloc};
use cranelift_codegen::isa::TargetIsa;

// TODO: in some areas, a bump allocator could be used to quickly allocate some vectors.

#[derive(Debug)]
pub enum Error {
    /// WebAssembly translation error.
    WasmError(WasmError),
    /// Code generation error.
    CodegenError(CodegenError),
    /// Memory error.
    MemoryError(MemoryError),
    /// No start specified.
    NoStart,
}

struct CompileResult {
    isa: Box<dyn TargetIsa>,
    contexts: Vec<Context>,
    start_func: Option<FuncIndex>,
    function_imports: Vec<FunctionImport>,
    total_size: usize,
}

impl CompileResult {
    /// Emit and link.
    pub fn emit_and_link(&self) -> Result<Thread, Error> {
        let start_func = self.start_func.ok_or(Error::NoStart)?;
        let defined_function_offset = self.function_imports.len();

        // Create code area, will be made executable read-only later.
        let code_vma = {
            let len = align_up(self.total_size);
            let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
            Vma::create(len)
                .and_then(|x| x.map(0, len, flags))
                .map_err(Error::MemoryError)?
        };
        let heap_vma = {
            // TODO: can max size be limited by wasm somehow?
            let len = HEAP_SIZE + HEAP_GUARD_SIZE;
            let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
            Vma::create(len as usize)
                .and_then(|x| Ok(x.map_lazily(flags)))
                .map_err(Error::MemoryError)?
        };

        // Emit code
        let capacity = self.contexts.len();
        let mut reloc_sinks: Vec<RelocSink> = Vec::with_capacity(capacity);
        let mut func_offsets: Vec<usize> = Vec::with_capacity(capacity);
        let mut offset: usize = 0;
        for context in &self.contexts {
            let mut reloc_sink = RelocSink::new();
            let mut trap_sink = NullTrapSink {};
            let mut null_stackmap_sink = NullStackmapSink {};

            let info = unsafe {
                let ptr = (code_vma.address() + offset).as_mut();

                context.emit_to_memory(
                    &*self.isa,
                    ptr,
                    &mut reloc_sink,
                    &mut trap_sink,
                    &mut null_stackmap_sink,
                )
            };

            func_offsets.push(offset);
            reloc_sinks.push(reloc_sink);

            offset += info.total_size as usize;
        }

        // Relocations
        for (idx, reloc_sink) in reloc_sinks.iter().enumerate() {
            for relocation in &reloc_sink.relocations {
                let reloc_addr = code_vma.address().as_usize()
                    + func_offsets[idx]
                    + relocation.code_offset as usize;

                // Determine target address.
                let target_off = match relocation.target {
                    RelocationTarget::UserFunction(target_idx) => {
                        func_offsets[target_idx.as_u32() as usize - defined_function_offset]
                    }
                    RelocationTarget::LibCall(_libcall) => unimplemented!(),
                    RelocationTarget::JumpTable(_jt) => unimplemented!(),
                };

                // Relocate!
                match relocation.reloc {
                    Reloc::X86PCRel4 | Reloc::X86CallPCRel4 => {
                        let delta = target_off
                            .wrapping_sub(func_offsets[idx] + relocation.code_offset as usize)
                            .wrapping_add(relocation.addend as usize);

                        unsafe {
                            write_unaligned(reloc_addr as *mut u32, delta as u32);
                        }
                    }
                    Reloc::Abs8 => {
                        let delta = target_off.wrapping_add(relocation.addend as usize);

                        unsafe {
                            write_unaligned(reloc_addr as *mut u64, delta as u64);
                        }
                    }
                    _ => unimplemented!(),
                }
            }
        }

        // Now the code is written, change it to read-only & executable.
        {
            let mut mapping = ActiveMapping::get();
            let flags = EntryFlags::PRESENT;
            mapping
                .change_flags_range(code_vma.address(), code_vma.size(), flags)
                .map_err(Error::MemoryError)?;
        };

        // Create the vm context.
        let mut vmctx_container = unsafe {
            VmContextContainer::new(heap_vma.address(), self.function_imports.len() as u32)
        };

        // Resolve import addresses.
        for (i, import) in self.function_imports.iter().enumerate() {
            println!("{} {:?}", i, import);

            // TODO: improve this
            match import.module.as_str() {
                "os" => {
                    // TODO: hardcoded to a fixed function atm
                    vmctx_container
                        .set_function_import(i as u32, VirtAddr::new(test_func as usize));
                    // TODO
                }
                _ => unimplemented!(),
            }
        }

        let start_offset = func_offsets[start_func.as_u32() as usize - defined_function_offset];
        let start_addr = code_vma.address().as_usize() + start_offset;

        Ok(Thread::create(
            VirtAddr::new(start_addr),
            code_vma,
            heap_vma,
            vmctx_container,
        )
        .map_err(Error::MemoryError)?)
    }
}

pub fn test() -> Result<(), Error> {
    let compile_result = compile()?;
    let thread = compile_result.emit_and_link()?;

    add_and_schedule_thread(thread);

    Ok(())
}

fn test_func(_vmctx: *const VmContext, param: i32) {
    let id = with_core_scheduler(|scheduler| scheduler.get_current_thread().id());
    println!("{:?}    called here {} {:#p}", id, param, _vmctx);
    //arch::halt();
}

fn compile() -> Result<CompileResult, Error> {
    // Hardcoded test
    let buffer = [
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0c, 0x03, 0x60, 0x01, 0x7f, 0x00,
        0x60, 0x00, 0x01, 0x7f, 0x60, 0x00, 0x00, 0x02, 0x0c, 0x01, 0x02, 0x6f, 0x73, 0x05, 0x68,
        0x65, 0x6c, 0x6c, 0x6f, 0x00, 0x00, 0x03, 0x03, 0x02, 0x01, 0x02, 0x08, 0x01, 0x02, 0x0a,
        0x13, 0x02, 0x05, 0x00, 0x41, 0xd2, 0x09, 0x0b, 0x0b, 0x00, 0x03, 0x40, 0x10, 0x01, 0x10,
        0x00, 0x0c, 0x00, 0x0b, 0x0b,
    ];

    let isa_builder = cranelift_native::builder().unwrap();
    let mut flag_builder = settings::builder();

    // Flags
    flag_builder.set("opt_level", "speed_and_size").unwrap();
    flag_builder.set("enable_probestack", "true").unwrap();

    let flags = settings::Flags::new(flag_builder);
    let isa = isa_builder.finish(flags);

    // Module
    let mut env = ModuleEnv::new(isa.frontend_config());
    let translation = translate_module(&buffer, &mut env).map_err(Error::WasmError)?;
    let defined_function_offset = env.function_imports.len();

    // Compile the functions and store their contexts.
    let mut contexts: Vec<Context> = Vec::with_capacity(env.func_bodies.len());
    let mut total_size: usize = 0;
    for idx in 0..env.func_bodies.len() {
        let mut ctx = Context::new();
        ctx.func.signature =
            env.get_sig(FuncIndex::from_u32((idx + defined_function_offset) as u32));

        let FunctionBody { body, offset } = env.func_bodies[idx];

        let mut func_trans = FuncTranslator::new();
        func_trans
            .translate(
                &translation,
                body,
                offset,
                &mut ctx.func,
                &mut FuncEnv::new(&env),
            )
            .map_err(Error::WasmError)?;

        //println!("{:?}", ctx.func);

        let info = ctx.compile(&*isa).map_err(Error::CodegenError)?;
        total_size += info.total_size as usize;
        contexts.push(ctx);
    }

    Ok(CompileResult {
        isa,
        contexts,
        start_func: env.start_func,
        function_imports: env.function_imports,
        total_size,
    })
}
