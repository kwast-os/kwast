//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr::{copy_nonoverlapping, write_unaligned};
use cranelift_codegen::binemit::{NullStackMapSink, NullTrapSink, Reloc};
use cranelift_codegen::ir::{types, LibCall, Signature, Type};
use cranelift_codegen::isa::{CallConv, TargetIsa};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::{CodegenError, Context};
use cranelift_wasm::{translate_module, Global, Memory, SignatureIndex};
use cranelift_wasm::{FuncIndex, FuncTranslator, WasmError};

use crate::arch::address::{align_up, VirtAddr};
use crate::arch::paging::EntryFlags;
use crate::mm::mapper::MemoryError;
use crate::mm::mapper::MemoryMapper;
use crate::mm::vma_allocator::{LazilyMappedVma, MappableVma, MappedVma};
use crate::tasking::protection_domain::ProtectionDomain;
use crate::tasking::scheduler::{add_and_schedule_thread, thread_exit, with_current_thread};
use crate::tasking::thread::Thread;
use crate::wasm::func_env::FuncEnv;
use crate::wasm::module_env::{
    DataInitializer, Export, FunctionBody, FunctionImport, ModuleEnv, TableElements,
};
use crate::wasm::reloc_sink::{RelocSink, RelocationTarget};
use crate::wasm::runtime::{
    runtime_memory_grow, runtime_memory_size, RUNTIME_MEMORY_GROW_IDX, RUNTIME_MEMORY_SIZE_IDX,
};
use crate::wasm::table::Table;
use crate::wasm::vmctx::{
    VmContext, VmContextContainer, VmFunctionImportEntry, VmTableElement, HEAP_GUARD_SIZE,
    HEAP_SIZE, WASM_PAGE_SIZE,
};
use crate::wasm::wasi::get_address_for_wasi_and_validate_sig;
use core::mem;

pub const WASM_VMCTX_TYPE: Type = types::I64;
pub const WASM_CALL_CONV: CallConv = CallConv::SystemV;

extern "C" {
    pub fn __rust_probestack();
}

static PROBESTACK: unsafe extern "C" fn() = __rust_probestack;

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
    /// Missing import
    MissingImport,
}

struct CompileResult<'data> {
    isa: Box<dyn TargetIsa>,
    contexts: Box<[Context]>,
    start_func: Option<FuncIndex>,
    func_sigs: Box<[SignatureIndex]>,
    signatures: Box<[Signature]>,
    memories: Box<[Memory]>,
    data_initializers: Box<[DataInitializer<'data>]>,
    function_imports: Box<[FunctionImport]>,
    tables: Box<[cranelift_wasm::Table]>,
    table_elements: Box<[TableElements]>,
    globals: Box<[Global]>,
    total_size: usize,
}

struct Instantiation<'r, 'data> {
    compile_result: &'r CompileResult<'data>,
    func_offsets: Vec<usize>,
}

struct WasmInstance {
    code_vma: MappedVma,
    heap_vma: LazilyMappedVma,
    vmctx_container: VmContextContainer,
    start_address: VirtAddr,
}

impl<'data> CompileResult<'data> {
    /// Compile result to instantiation.
    pub fn instantiate(&self) -> Instantiation {
        Instantiation::new(self)
    }

    /// Gets the signature of a function.
    pub fn get_sig(&self, func_idx: FuncIndex) -> &Signature {
        let sig_idx = self.func_sigs[func_idx.as_u32() as usize];
        &self.signatures[sig_idx.as_u32() as usize]
    }
}

impl<'r, 'data> Instantiation<'r, 'data> {
    /// Creates a new instantiation.
    fn new(compile_result: &'r CompileResult<'data>) -> Self {
        let capacity = compile_result.contexts.len();

        Self {
            compile_result,
            func_offsets: Vec::with_capacity(capacity),
        }
    }

    /// Gets the offset of the defined functions in the function array.
    fn defined_function_offset(&self) -> usize {
        self.compile_result.function_imports.len()
    }

    // Helper to get  the function address from a function index.
    fn get_func_address(&self, code_vma: &MappedVma, index: FuncIndex) -> VirtAddr {
        let offset = self.func_offsets[index.as_u32() as usize - self.defined_function_offset()];
        VirtAddr::new(code_vma.address().as_usize() + offset)
    }

    /// Emit code.
    fn emit(&mut self) -> Result<(MappedVma, LazilyMappedVma, Vec<RelocSink>), Error> {
        let (code_vma, heap_vma) = with_current_thread(|thread| {
            thread.domain().with(|vma, mapping| {
                // Create code area, will be made executable read-only later.
                let code_vma = {
                    let len = align_up(self.compile_result.total_size);
                    let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;

                    vma.create_vma(len)
                        .and_then(|v| v.map(mapping, 0, len, flags))
                        .map_err(Error::MemoryError)?
                };

                let heap_vma = {
                    let mem = self.compile_result.memories.get(0).unwrap_or(&Memory {
                        minimum: 0,
                        maximum: None,
                        shared: false,
                    });
                    let minimum = mem.minimum as usize * WASM_PAGE_SIZE;

                    // Note: func_env assumes 4GiB is available, also makes it so that we can't construct
                    //       a pointer outside (See issue #10 also)
                    let maximum = HEAP_SIZE;

                    if minimum as u64 > HEAP_SIZE || maximum > HEAP_SIZE {
                        return Err(Error::MemoryError(MemoryError::InvalidRange));
                    }

                    let len = maximum + HEAP_GUARD_SIZE;
                    let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
                    vma.create_vma(len as usize)
                        .and_then(|v| v.map_lazily(mapping, minimum, flags))
                        .map_err(Error::MemoryError)?
                };

                Ok((code_vma, heap_vma))
            })
        })?;

        // Emit code
        let capacity = self.compile_result.contexts.len();
        let mut reloc_sinks: Vec<RelocSink> = Vec::with_capacity(capacity);
        let mut offset: usize = 0;

        for context in self.compile_result.contexts.iter() {
            let mut reloc_sink = RelocSink::new();
            let mut trap_sink = NullTrapSink {};
            let mut null_stackmap_sink = NullStackMapSink {};

            let info = unsafe {
                let ptr = (code_vma.address() + offset).as_mut();

                context.emit_to_memory(
                    &*self.compile_result.isa,
                    ptr,
                    &mut reloc_sink,
                    &mut trap_sink,
                    &mut null_stackmap_sink,
                )
            };

            self.func_offsets.push(offset);
            reloc_sinks.push(reloc_sink);

            offset += info.total_size as usize;
        }

        //println!("{:x}", offset);

        Ok((code_vma, heap_vma, reloc_sinks))
    }

    /// Emit and link.
    pub fn emit_and_link(mut self) -> Result<WasmInstance, Error> {
        let defined_function_offset = self.defined_function_offset();
        let (code_vma, heap_vma, reloc_sinks) = self.emit()?;

        // Relocations
        for (idx, reloc_sink) in reloc_sinks.iter().enumerate() {
            for relocation in &reloc_sink.relocations {
                let reloc_addr = code_vma.address().as_usize()
                    + self.func_offsets[idx]
                    + relocation.code_offset as usize;

                // Determine target address.
                let target_off = match relocation.target {
                    RelocationTarget::UserFunction(target_idx) => {
                        self.func_offsets[target_idx.as_u32() as usize - defined_function_offset]
                    }
                    RelocationTarget::RuntimeFunction(idx) => match idx {
                        RUNTIME_MEMORY_GROW_IDX => runtime_memory_grow as usize,
                        RUNTIME_MEMORY_SIZE_IDX => runtime_memory_size as usize,
                        _ => unreachable!(),
                    },
                    RelocationTarget::LibCall(libcall) => match libcall {
                        LibCall::Probestack => PROBESTACK as usize,
                        _ => unimplemented!("{:?}", libcall),
                    },
                    // Not necessary unless we split rodata and code
                    //RelocationTarget::JumpTable(jt) => {
                    //    let ctx = &self.compile_result.contexts[idx];
                    //    let offset = ctx
                    //        .func
                    //        .jt_offsets
                    //        .get(jt)
                    //        .expect("jump table should exist");
                    //    self.func_offsets[idx] + *offset as usize
                    //}
                };

                // Relocate!
                match relocation.reloc {
                    Reloc::X86PCRel4 | Reloc::X86CallPCRel4 => {
                        let delta = target_off
                            .wrapping_sub(self.func_offsets[idx] + relocation.code_offset as usize)
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
                    Reloc::X86PCRelRodata4 => { /* ignore */ }
                    _ => unimplemented!("{:?}", relocation),
                }
            }
        }

        // Now the code is written, change it to read-only & executable.
        with_current_thread(|thread| {
            thread.domain().with(|_vma, mapping| {
                let flags = EntryFlags::PRESENT;
                mapping
                    .change_flags_range(code_vma.address(), code_vma.size(), flags)
                    .map_err(Error::MemoryError)
            })
        })?;

        let start_func = self.compile_result.start_func.ok_or(Error::NoStart)?;
        let start_address = self.get_func_address(&code_vma, start_func);

        let vmctx_container = self.create_vmctx_container(&code_vma, &heap_vma)?;

        Ok(WasmInstance {
            code_vma,
            heap_vma,
            vmctx_container,
            start_address,
        })
    }

    /// Creates the VmContext container.
    fn create_vmctx_container(
        &self,
        code_vma: &MappedVma,
        heap_vma: &LazilyMappedVma,
    ) -> Result<VmContextContainer, Error> {
        // Create the vm context.
        let mut vmctx_container = {
            // Initialize table vectors.
            let tables: Vec<Table> = self
                .compile_result
                .tables
                .iter()
                .map(|x| Table::new(x))
                .collect();

            unsafe {
                VmContextContainer::new(
                    heap_vma.address(),
                    self.compile_result.globals.len() as u32,
                    self.compile_result.function_imports.len() as u32,
                    tables,
                )
            }
        };

        // Resolve import addresses.
        {
            // Safety: we are the only ones who have access to this slice right now.
            let function_imports = unsafe { vmctx_container.function_imports_as_mut_slice() };

            for (entry, (i, import)) in function_imports
                .iter_mut()
                .zip(self.compile_result.function_imports.iter().enumerate())
            {
                println!("{} {:?}", i, import);

                let sig = self.compile_result.get_sig(FuncIndex::from_u32(i as u32));

                *entry = match import.module.as_str() {
                    "wasi_snapshot_preview1" => VmFunctionImportEntry {
                        address: get_address_for_wasi_and_validate_sig(&import.field, sig)
                            .ok_or(Error::MissingImport)?,
                    },
                    _ => unimplemented!(), // TODO
                };
            }
        }

        // Create tables.
        {
            // Fill in the tables.
            for elements in self.compile_result.table_elements.iter() {
                // TODO: support this and verify bounds?
                assert!(elements.base.is_none(), "not implemented yet");

                let offset = elements.offset;
                let table = vmctx_container.get_table(elements.index);

                for (i, func_idx) in elements.elements.iter().enumerate() {
                    table.set(
                        i + offset,
                        VmTableElement::new(
                            self.get_func_address(code_vma, *func_idx),
                            self.compile_result.func_sigs[func_idx.as_u32() as usize],
                        ),
                    );
                }
            }
        }

        // Run data initializers
        {
            for initializer in self.compile_result.data_initializers.iter() {
                assert_eq!(initializer.memory_index.as_u32(), 0);
                // TODO: support this
                assert!(initializer.base.is_none());

                let offset = initializer.offset;

                if offset > heap_vma.size() - initializer.data.len() {
                    return Err(Error::MemoryError(MemoryError::InvalidRange));
                }

                let offset = heap_vma.address() + offset;

                //println!(
                //    "Copy {:?} to {:?} length {}",
                //    initializer.data.as_ptr(),
                //    offset.as_mut::<u8>(),
                //    initializer.data.len()
                //);
                unsafe {
                    copy_nonoverlapping(
                        initializer.data.as_ptr(),
                        offset.as_mut::<u8>(),
                        initializer.data.len(),
                    );
                }
            }

            vmctx_container.write_tables_to_vmctx();
        }

        // Create globals
        {
            for (i, global) in self.compile_result.globals.iter().enumerate() {
                // Safety: valid index
                unsafe {
                    vmctx_container.set_global(i as u32, &global);
                }
            }
        }

        Ok(vmctx_container)
    }
}

/// Runs WebAssembly from a buffer.
pub fn run(buffer: &[u8], domain: ProtectionDomain) -> Result<(), Error> {
    let compile_result = Box::new(compile(buffer)?);
    let compile_result = Box::into_raw(compile_result);
    // Safety: valid and correct entry point.
    let thread = unsafe {
        Thread::create(
            domain,
            VirtAddr::new(start_from_compile_result as usize),
            compile_result as usize,
        )
        .map_err(Error::MemoryError)?
    };
    add_and_schedule_thread(thread);

    Ok(())
}

/// Start the wasm application from compile result.
extern "C" fn start_from_compile_result(compile_result: *mut CompileResult) {
    let compile_result = unsafe { Box::from_raw(compile_result) };
    let instantiation = compile_result.instantiate();

    match instantiation.emit_and_link() {
        Ok(wasm_instance) => {
            let vmctx = wasm_instance.vmctx_container.ptr();

            let func: extern "C" fn(*const VmContext) =
                unsafe { mem::transmute(wasm_instance.start_address.as_usize()) };

            with_current_thread(|thread| {
                // Safety: this is a new thread without existing wasm data.
                unsafe {
                    thread.set_wasm_data(
                        wasm_instance.code_vma,
                        wasm_instance.heap_vma,
                        wasm_instance.vmctx_container,
                    )
                }
            });

            drop(compile_result);

            func(vmctx);
        }

        Err(e) => {
            drop(compile_result);
            println!("Error while starting: {:?}", e);
        }
    }

    thread_exit(0);
}

/// Compiles a WebAssembly buffer.
fn compile(buffer: &[u8]) -> Result<CompileResult, Error> {
    let isa_builder =
        cranelift_native::builder().expect("native flag builder should be constructable");
    let mut flag_builder = settings::builder();

    // Flags
    flag_builder
        .set("opt_level", "speed_and_size")
        .expect("valid flag");
    flag_builder
        .set("enable_probestack", "true")
        .expect("valid flag");
    flag_builder.set("enable_simd", "true").expect("valid flag");
    // TODO: avoid div traps?

    let flags = settings::Flags::new(flag_builder);
    // println!("{}", flags.to_string());
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
            env.get_sig_from_func(FuncIndex::from_u32((idx + defined_function_offset) as u32));

        print!("\r compiling {:?}", idx);

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
        total_size += info.total_size as usize;
        contexts.push(ctx);
    }

    // Determine start function. If it's not given, search for "_start" as specified by WASI.
    let start_func = env.start_func.or_else(|| match env.exports.get("_start") {
        Some(Export::Function(idx)) => Some(*idx),
        _ => None,
    });

    let compile_result = CompileResult {
        isa,
        contexts: contexts.into_boxed_slice(),
        memories: env.memories.into_boxed_slice(),
        func_sigs: env.func_sigs.into_boxed_slice(),
        data_initializers: env.data_initializers.into_boxed_slice(),
        start_func,
        function_imports: env.function_imports.into_boxed_slice(),
        tables: env.tables.into_boxed_slice(),
        table_elements: env.table_elements.into_boxed_slice(),
        globals: env.globals.into_boxed_slice(),
        total_size,
        signatures: env.signatures.into_boxed_slice(),
    };

    // Check the signature of the start function.
    // Must not take any arguments (which means arg length == 1 because vmctx)
    // and not have return values.
    if let Some(start_func) = start_func {
        let sig = compile_result.get_sig(start_func);

        if !sig.returns.is_empty() || sig.params.len() != 1 {
            return Err(Error::NoStart);
        }
    }

    Ok(compile_result)
}
