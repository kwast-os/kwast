//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use cranelift_codegen::{Context, CodegenError};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_wasm::{FuncTranslator, WasmError, FuncIndex};
use cranelift_wasm::translate_module;
use cranelift_native;

use alloc::vec::Vec;
use crate::wasm::module_env::ModuleEnv;
use crate::wasm::func_env::FuncEnv;
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::{MemoryMapper, MappingError};
use crate::arch::address::VirtAddr;
use cranelift_codegen::isa::TargetIsa;
use alloc::boxed::Box;
use crate::wasm::reloc_sink::{RelocSink, RelocationTarget};
use cranelift_codegen::binemit::{NullTrapSink, NullStackmapSink, Reloc};
use core::intrinsics::transmute;
use bitflags::_core::ptr::write_unaligned;

// TODO: in some areas, a bump allocator may be used to quickly allocate some vectors.

#[derive(Debug)]
pub enum Error {
    /// WebAssembly translation error.
    WasmError(WasmError),
    /// Code generation error.
    CodegenError(CodegenError),
    /// Memory error.
    MemoryError(MappingError),
}

struct CompileResult {
    isa: Box<dyn TargetIsa>,
    contexts: Vec<Context>,
    total_size: usize,
}

#[derive(Debug)]
struct VMContext {
    heap_base: usize,
}

pub fn test() -> Result<(), Error> {// TODO: make better
    let compile_result = compile()?;

    // TODO
    let addr = 256 * 1024 * 1024;
    ActiveMapping::get()
        .map_range(VirtAddr::new(addr), compile_result.total_size, EntryFlags::PRESENT | EntryFlags::WRITABLE)
        .map_err(|map_error| Error::MemoryError(map_error))?;

    // TODO: change flags method & expose that also to the boot
    // TODO: ! make sure to protect rodata

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
            let ptr = (addr + offset) as *mut u8;

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
            let reloc_addr = addr + func_offsets[idx] + relocation.code_offset as usize;

            // Determine target address.
            let target_off = match relocation.target {
                RelocationTarget::UserFunction(target_idx) => { // TODO: must be defined, not imported?
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

                    println!("{}, {}, {}", target_off, delta as isize, relocation.addend);
                }
                Reloc::Abs8 => {
                    let delta = target_off.wrapping_add(relocation.addend as usize);

                    unsafe {
                        write_unaligned(reloc_addr as *mut u64, delta as u64);
                    }
                }
                _ => unimplemented!()
            }
        }
    }

    // TODO: test
    for x in 0..compile_result.total_size {
        unsafe {
            let ptr = (addr + x) as *mut u8;
            print!("{:#x}, ", *ptr);
        }
    }

    println!();
    println!("this is {:#x}", test as *const () as usize);
    println!("now going to execute code");


    let vmctx = VMContext {
        heap_base: addr + 0x500, // TODO
    };

    let ptr = addr as *const ();
    let code: extern "C" fn(i32, i32, &VMContext) -> () = unsafe { transmute(ptr) };

    println!("execution returned this result: {:?}", code(4, 5, &vmctx)); // write fibonacci(5) to 0x500+4
    println!("{}", unsafe { *((vmctx.heap_base + 4) as *const i32) });

    Ok(())
}

fn compile() -> Result<CompileResult, Error> {
    // Hardcoded test
    /*let buffer = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60,
        0x02, 0x7f, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01,
        0x03, 0x73, 0x75, 0x6d, 0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20,
        0x00, 0x20, 0x01, 0x6a, 0x0b, 0x00, 0x16, 0x04, 0x6e, 0x61, 0x6d, 0x65,
        0x01, 0x06, 0x01, 0x00, 0x03, 0x73, 0x75, 0x6d, 0x02, 0x07, 0x01, 0x00,
        0x02, 0x00, 0x00, 0x01, 0x00];*/
    /*let buffer = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60,
        0x01, 0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03,
        0x66, 0x61, 0x63, 0x00, 0x00, 0x0a, 0x19, 0x01, 0x17, 0x00, 0x20, 0x00,
        0x41, 0x02, 0x49, 0x04, 0x7f, 0x41, 0x01, 0x05, 0x20, 0x00, 0x20, 0x00,
        0x41, 0x01, 0x6b, 0x10, 0x00, 0x6c, 0x0b, 0x0b];*/
    let buffer = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0b, 0x02, 0x60,
        0x02, 0x7f, 0x7f, 0x00, 0x60, 0x01, 0x7f, 0x01, 0x7f, 0x03, 0x03, 0x02,
        0x00, 0x01, 0x05, 0x03, 0x01, 0x00, 0x01, 0x07, 0x17, 0x01, 0x13, 0x72,
        0x65, 0x63, 0x75, 0x72, 0x73, 0x69, 0x76, 0x65, 0x5f, 0x66, 0x69, 0x62,
        0x6f, 0x6e, 0x61, 0x63, 0x63, 0x69, 0x00, 0x01, 0x0a, 0x2a, 0x02, 0x0b,
        0x00, 0x20, 0x00, 0x20, 0x01, 0x10, 0x01, 0x36, 0x02, 0x00, 0x0b, 0x1c,
        0x00, 0x20, 0x00, 0x41, 0x02, 0x49, 0x04, 0x7f, 0x41, 0x01, 0x05, 0x20,
        0x00, 0x41, 0x01, 0x6b, 0x10, 0x01, 0x20, 0x00, 0x41, 0x02, 0x6b, 0x10,
        0x01, 0x6a, 0x0b, 0x0b];

    let isa_builder = cranelift_native::builder().unwrap();
    let mut flag_builder = settings::builder();

    // Flags
    flag_builder.set("opt_level", "speed").unwrap();
    println!("a");

    let flags = settings::Flags::new(flag_builder);
    let isa = isa_builder.finish(flags);
    println!("b");

    // Module
    let mut env = ModuleEnv::new(isa.frontend_config());
    let translation = translate_module(&buffer, &mut env)
        .map_err(|e| Error::WasmError(e))?;

    // Compile the functions and store their contexts.
    let mut contexts: Vec<Context> = Vec::with_capacity(env.func_bodies.len());
    let mut total_size: usize = 0;
    for idx in 0..env.func_bodies.len() {
        let mut ctx = Context::new();
        ctx.func.signature = env.get_sig(FuncIndex::from_u32(idx as u32));

        let mut func_trans = FuncTranslator::new();
        func_trans.translate(
            &translation,
            &env.func_bodies[idx],
            0,
            &mut ctx.func,
            &mut FuncEnv::new(&env),
        )
            .map_err(|e| Error::WasmError(e))?;

        let info = ctx.compile(&*isa).map_err(|e| Error::CodegenError(e))?;
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
