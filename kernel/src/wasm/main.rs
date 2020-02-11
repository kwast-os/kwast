//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::{CodegenError, Context};
use cranelift_wasm::translate_module;
use cranelift_wasm::{FuncIndex, FuncTranslator, WasmError};

use crate::arch::address::align_up;
use crate::arch::paging::ActiveMapping;
use crate::arch::paging::EntryFlags;
use crate::arch::x86_64::paging::PAGE_SIZE;
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::mm::vma_allocator::Vma;
use crate::wasm::func_env::FuncEnv;
use crate::wasm::module_env::{FunctionBody, ModuleEnv};
use crate::wasm::reloc_sink::{RelocSink, RelocationTarget};
use crate::wasm::vmctx::{VMContext, HEAP_GUARD_SIZE, HEAP_SIZE};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::intrinsics::transmute;
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
}

struct CompileResult {
    isa: Box<dyn TargetIsa>,
    contexts: Vec<Context>,
    total_size: usize,
}

/*struct TrapSink {
}

impl binemit::TrapSink for TrapSink {
    fn trap(&mut self, a: u32, b: SourceLoc, c: TrapCode) {
        println!("{:?} {:?} {:?}", a, b, c);
    }
}*/

pub fn test() -> Result<(), Error> {
    // TODO: make better
    let compile_result = compile()?;

    // Create virtual memory areas.
    let code_vma = {
        let len = align_up(compile_result.total_size);
        Vma::create(len).map_err(Error::MemoryError)?
    };
    let heap_vma = {
        // TODO: can max size be limited by wasm somehow?
        let len = HEAP_SIZE + HEAP_GUARD_SIZE;
        Vma::create(len as usize).map_err(Error::MemoryError)?
    };

    let mut mapping = ActiveMapping::get();

    // Map writable section for code. We will later change this to read-only.
    {
        let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
        mapping
            .map_range(code_vma.address(), code_vma.size(), flags)
            .map_err(Error::MemoryError)?;
    };

    // TODO: change flags method & expose that also to the boot

    // Emit code
    let capacity = compile_result.contexts.len();
    let mut reloc_sinks: Vec<RelocSink> = Vec::with_capacity(capacity);
    let mut func_offsets: Vec<usize> = Vec::with_capacity(capacity);
    let mut offset: usize = 0;
    for context in &compile_result.contexts {
        let mut reloc_sink = RelocSink::new();
        let mut trap_sink = NullTrapSink {};
        let mut null_stackmap_sink = NullStackmapSink {};

        let info = unsafe {
            let ptr = (code_vma.address() + offset).as_mut();

            context.emit_to_memory(
                &*compile_result.isa,
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
            let reloc_addr =
                code_vma.address().as_usize() + func_offsets[idx] + relocation.code_offset as usize;

            // Determine target address.
            let target_off = match relocation.target {
                RelocationTarget::UserFunction(target_idx) => {
                    // TODO: must be defined, not imported?
                    func_offsets[target_idx.as_u32() as usize]
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

    // Now the code is written, change it to read-only.
    {
        let flags = EntryFlags::PRESENT;
        mapping
            .change_flags_range(code_vma.address(), code_vma.size(), flags)
            .map_err(Error::MemoryError)?;
    };

    for x in 0..compile_result.total_size {
        unsafe {
            let ptr = (code_vma.address().as_usize() + x) as *mut u8;
            print!("{:#x}, ", *ptr);
        }
    }

    println!();
    println!("now going to execute code"); // TODO: should happen in a process

    let vmctx = VMContext {
        heap_base: heap_vma.address(),
    };

    // TODO: do this properly (allocate on access)
    {
        let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
        mapping
            .map_range(heap_vma.address(), PAGE_SIZE, flags)
            .unwrap();
    }

    let ptr = code_vma.address().as_usize() as *const ();
    let code: extern "C" fn(i32, i32, &VMContext) -> () = unsafe { transmute(ptr) };
    code(4, 10, &vmctx); // write fibonacci(10) to 0x500+4
    println!("execution stopped, reading: {}", unsafe {
        *((vmctx.heap_base.as_usize() + 4) as *const i32)
    });

    Ok(())
}

fn compile() -> Result<CompileResult, Error> {
    // Hardcoded test
    /*let buffer = [
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60,
        0x01, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x0c, 0x01, 0x08,
        0x6f, 0x76, 0x65, 0x72, 0x66, 0x6c, 0x6f, 0x77, 0x00, 0x00, 0x0a, 0x0b,
        0x01, 0x09, 0x00, 0x20, 0x00, 0x10, 0x00, 0x20, 0x00, 0x6a, 0x0b
    ];*/
    /*let buffer = [
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03,
        0x02, 0x01, 0x00, 0x05, 0x03, 0x01, 0x00, 0x01, 0x07, 0x08, 0x01, 0x04, 0x74, 0x65, 0x73,
        0x74, 0x00, 0x00, 0x0a, 0x0f, 0x01, 0x0d, 0x00, 0x41, 0x7f, 0x41, 0xad, 0xbd, 0xb7, 0xf5,
        0x7d, 0x36, 0x02, 0x00, 0x0b,
    ];*/
    let buffer = [
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0b, 0x02, 0x60, 0x02, 0x7f, 0x7f,
        0x00, 0x60, 0x01, 0x7f, 0x01, 0x7f, 0x03, 0x03, 0x02, 0x00, 0x01, 0x05, 0x03, 0x01, 0x00,
        0x01, 0x07, 0x17, 0x01, 0x13, 0x72, 0x65, 0x63, 0x75, 0x72, 0x73, 0x69, 0x76, 0x65, 0x5f,
        0x66, 0x69, 0x62, 0x6f, 0x6e, 0x61, 0x63, 0x63, 0x69, 0x00, 0x01, 0x0a, 0x2a, 0x02, 0x0b,
        0x00, 0x20, 0x00, 0x20, 0x01, 0x10, 0x01, 0x36, 0x02, 0x00, 0x0b, 0x1c, 0x00, 0x20, 0x00,
        0x41, 0x02, 0x49, 0x04, 0x7f, 0x41, 0x01, 0x05, 0x20, 0x00, 0x41, 0x01, 0x6b, 0x10, 0x01,
        0x20, 0x00, 0x41, 0x02, 0x6b, 0x10, 0x01, 0x6a, 0x0b, 0x0b,
    ];

    let isa_builder = cranelift_native::builder().unwrap();
    let mut flag_builder = settings::builder();

    // Flags
    flag_builder.set("opt_level", "speed_and_size").unwrap();
    //flag_builder.set("enable_probestack", "true").unwrap();

    let flags = settings::Flags::new(flag_builder);
    let isa = isa_builder.finish(flags);

    // Module
    let mut env = ModuleEnv::new(isa.frontend_config());
    let translation = translate_module(&buffer, &mut env).map_err(Error::WasmError)?;

    // Compile the functions and store their contexts.
    let mut contexts: Vec<Context> = Vec::with_capacity(env.func_bodies.len());
    let mut total_size: usize = 0;
    for idx in 0..env.func_bodies.len() {
        let mut ctx = Context::new();
        ctx.func.signature = env.get_sig(FuncIndex::from_u32(idx as u32));

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

        let info = ctx.compile(&*isa).map_err(Error::CodegenError)?;
        //println!("code_size: {}, rodata_size: {}, total_size: {}", info.code_size, info.rodata_size, info.total_size);
        total_size += info.total_size as usize;
        contexts.push(ctx);
    }

    Ok(CompileResult {
        isa,
        contexts,
        total_size,
    })
}
