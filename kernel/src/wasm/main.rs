//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::{CodegenError, Context};
use cranelift_wasm::translate_module;
use cranelift_wasm::{FuncIndex, FuncTranslator, WasmError};

use crate::arch::address::{align_up, VirtAddr};
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::mm::vma_allocator::{LazilyMappedVma, MappableVma, MappedVma, Vma};
use crate::tasking::scheduler::{add_and_schedule_thread, with_core_scheduler};
use crate::tasking::thread::Thread;
use crate::wasm::func_env::FuncEnv;
use crate::wasm::module_env::{FunctionBody, FunctionImport, ModuleEnv, TableElements};
use crate::wasm::reloc_sink::{RelocSink, RelocationTarget};
use crate::wasm::table::Table;
use crate::wasm::vmctx::{
    VmContext, VmContextContainer, VmFunctionImportEntry, VmTableElement, HEAP_GUARD_SIZE,
    HEAP_SIZE,
};
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
    tables: Vec<cranelift_wasm::Table>,
    table_elements: Vec<TableElements>,
    total_size: usize,
}

struct Instantiation<'a> {
    compile_result: &'a CompileResult,
    func_offsets: Vec<usize>,
}

impl CompileResult {
    /// Compile result to instantiation.
    pub fn instantiate(&self) -> Instantiation {
        Instantiation::new(self)
    }
}

impl<'a> Instantiation<'a> {
    fn new(compile_result: &'a CompileResult) -> Self {
        let capacity = compile_result.contexts.len();

        Self {
            compile_result,
            func_offsets: Vec::with_capacity(capacity),
        }
    }

    fn defined_function_offset(&self) -> usize {
        self.compile_result.function_imports.len()
    }

    // Helper to get  the function address from a function index.
    fn get_func_address(&self, code_vma: &MappedVma, index: FuncIndex) -> VirtAddr {
        let offset = self.func_offsets[index.as_u32() as usize - self.defined_function_offset()];
        VirtAddr::new(code_vma.address().as_usize() + offset)
    }

    fn emit(&mut self) -> Result<(MappedVma, LazilyMappedVma, Vec<RelocSink>), Error> {
        // Create code area, will be made executable read-only later.
        let code_vma = {
            let len = align_up(self.compile_result.total_size);
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
        let capacity = self.compile_result.contexts.len();
        let mut reloc_sinks: Vec<RelocSink> = Vec::with_capacity(capacity);
        let mut offset: usize = 0;

        for context in &self.compile_result.contexts {
            let mut reloc_sink = RelocSink::new();
            let mut trap_sink = NullTrapSink {};
            let mut null_stackmap_sink = NullStackmapSink {};

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

        Ok((code_vma, heap_vma, reloc_sinks))
    }

    /// Emit and link.
    pub fn emit_and_link(&mut self) -> Result<Thread, Error> {
        let start_func = self.compile_result.start_func.ok_or(Error::NoStart)?;
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
                    RelocationTarget::LibCall(_libcall) => unimplemented!(),
                    RelocationTarget::JumpTable(_jt) => unimplemented!(),
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
                    _ => unimplemented!(),
                }
            }
        }

        // Debug code: print the bytes of the code section.
        for i in 0..self.compile_result.total_size {
            let address = code_vma.address().as_usize() + i;
            unsafe {
                let ptr = address as *const u8;
                print!("{:#x}, ", *ptr);
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

        let vmctx_container = self.create_vmctx_container(&code_vma, &heap_vma);

        Ok(Thread::create(
            self.get_func_address(&code_vma, start_func),
            code_vma,
            heap_vma,
            vmctx_container,
        )
        .map_err(Error::MemoryError)?)
    }

    fn create_vmctx_container(
        &self,
        code_vma: &MappedVma,
        heap_vma: &LazilyMappedVma,
    ) -> VmContextContainer {
        // Create the vm context.
        let mut vmctx_container = unsafe {
            VmContextContainer::new(
                heap_vma.address(),
                self.compile_result.function_imports.len() as u32,
                self.compile_result.tables.len() as u32,
            )
        };

        // Resolve import addresses.
        {
            let function_imports = unsafe { vmctx_container.function_imports_as_mut_slice() };
            for (i, import) in self.compile_result.function_imports.iter().enumerate() {
                println!("{} {:?}", i, import);

                // TODO: improve this
                match import.module.as_str() {
                    "os" => {
                        // TODO: hardcoded to a fixed function atm
                        function_imports[i] = VmFunctionImportEntry {
                            address: VirtAddr::new(test_func as usize),
                        };
                        // TODO
                    }
                    _ => unimplemented!(),
                }
            }
        }

        // Create tables.
        {
            // Initialize table vectors.
            let mut tables: Vec<Table> = self
                .compile_result
                .tables
                .iter()
                .map(|x| Table::new(x))
                .collect();

            // Fill in the tables.
            for elements in &self.compile_result.table_elements {
                // TODO: support this
                assert!(elements.base.is_none(), "not implemented yet");
                assert_eq!(elements.offset, 0, "not implemented yet");

                let offset = 0usize;
                let table = &mut tables[elements.index.as_u32() as usize];

                for (i, func_idx) in elements.elements.iter().enumerate() {
                    table.set(
                        i + offset,
                        VmTableElement {
                            address: self.get_func_address(code_vma, *func_idx),
                        },
                    );
                }
            }

            // TODO: keep tables in the vm context container
        }

        vmctx_container
    }
}

pub fn run(buffer: &[u8]) -> Result<(), Error> {
    let thread = {
        let compile_result = compile(buffer)?;
        let mut instantiation = compile_result.instantiate();
        instantiation.emit_and_link()?
    };

    add_and_schedule_thread(thread);

    Ok(())
}

fn test_func(_vmctx: *const VmContext, param: i32) {
    let id = with_core_scheduler(|scheduler| scheduler.get_current_thread().id());
    println!("{:?}    os hello {} {:#p}", id, param, _vmctx);
    //arch::halt();
}

fn compile(buffer: &[u8]) -> Result<CompileResult, Error> {
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
            env.get_sig_from_func(FuncIndex::from_u32((idx + defined_function_offset) as u32));

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

        // println!("{:?}", ctx.func);

        let info = ctx.compile(&*isa).map_err(Error::CodegenError)?;
        total_size += info.total_size as usize;
        contexts.push(ctx);
    }

    Ok(CompileResult {
        isa,
        contexts,
        start_func: env.start_func,
        function_imports: env.function_imports,
        tables: env.tables,
        table_elements: env.table_elements,
        total_size,
    })
}
