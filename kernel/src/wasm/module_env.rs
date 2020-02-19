//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use alloc::boxed::Box;
use alloc::vec::Vec;
use cranelift_codegen::ir::{AbiParam, ArgumentPurpose, Signature};
use cranelift_codegen::isa;
use cranelift_codegen::isa::TargetFrontendConfig;
use cranelift_wasm::{
    FuncIndex, Global, GlobalIndex, Memory, MemoryIndex, ModuleEnvironment, ModuleTranslationState,
    SignatureIndex, Table, TableIndex, TargetEnvironment, WasmError, WasmResult,
};

pub struct FunctionBody<'data> {
    pub body: &'data [u8],
    pub offset: usize,
}

pub struct ModuleEnv<'data> {
    /// Passed target configuration.
    cfg: isa::TargetFrontendConfig,
    /// Starting function.
    start_func: Option<FuncIndex>,
    /// Vector of all signatures.
    signatures: Vec<Signature>,
    /// Function types.
    func_types: Vec<SignatureIndex>,
    /// Function Wasm body contents.
    pub func_bodies: Vec<FunctionBody<'data>>,
    /// Memories.
    memories: Vec<Memory>,
    /// Keep track of the imported function count.
    imported_func_count: u32,
}

impl<'data> ModuleEnv<'data> {
    /// Creates a new module environment for the given configuration.
    pub fn new(cfg: TargetFrontendConfig) -> Self {
        Self {
            cfg,
            start_func: None,
            signatures: Vec::new(),
            func_types: Vec::new(),
            func_bodies: Vec::new(),
            memories: Vec::new(),
            imported_func_count: 0,
        }
    }

    /// Gets the signature of the given function.
    pub fn get_sig(&self, index: FuncIndex) -> Signature {
        let sig_index = self.func_types[index.as_u32() as usize];
        self.signatures[sig_index.as_u32() as usize].clone()
    }

    /// Returns whether the function index corresponds to an imported function.
    pub fn is_imported_func(&self, index: FuncIndex) -> bool {
        // Imported functions are defined first.
        index.as_u32() < self.imported_func_count
    }
}

impl<'data> TargetEnvironment for ModuleEnv<'data> {
    fn target_config(&self) -> TargetFrontendConfig {
        self.cfg
    }
}

impl<'data> ModuleEnvironment<'data> for ModuleEnv<'data> {
    fn reserve_signatures(&mut self, num: u32) -> WasmResult<()> {
        self.signatures.reserve_exact(num as usize);
        Ok(())
    }

    fn declare_signature(&mut self, mut sig: Signature) -> WasmResult<()> {
        sig.params.push(AbiParam::special(
            self.pointer_type(),
            ArgumentPurpose::VMContext,
        ));
        self.signatures.push(sig);
        Ok(())
    }

    fn declare_func_import(
        &mut self,
        sig_index: SignatureIndex,
        module: &'data str,
        field: &'data str,
    ) -> WasmResult<()> {
        println!("{:?} {} {}", sig_index, module, field);
        self.func_types.push(sig_index);
        self.imported_func_count += 1;
        // TODO: track module & field
        Ok(())
    }

    fn declare_table_import(
        &mut self,
        _table: Table,
        _module: &'data str,
        _field: &'data str,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_memory_import(
        &mut self,
        _memory: Memory,
        _module: &'data str,
        _field: &'data str,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_global_import(
        &mut self,
        _global: Global,
        _module: &'data str,
        _field: &'data str,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn reserve_func_types(&mut self, num: u32) -> WasmResult<()> {
        self.func_types.reserve_exact(num as usize);
        Ok(())
    }

    fn declare_func_type(&mut self, sig_index: SignatureIndex) -> WasmResult<()> {
        self.func_types.push(sig_index);
        Ok(())
    }

    fn declare_table(&mut self, _table: Table) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_memory(&mut self, memory: Memory) -> WasmResult<()> {
        // TODO: Shared memories and more than one memory not supported right now.
        assert_eq!(memory.shared, false);
        assert_eq!(self.memories.len(), 0);
        self.memories.push(memory);
        Ok(())
    }

    fn declare_global(&mut self, _global: Global) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_func_export(&mut self, _func_index: FuncIndex, name: &'data str) -> WasmResult<()> {
        println!("declare func export: {}", name);
        Ok(())
    }

    fn declare_table_export(
        &mut self,
        _table_index: TableIndex,
        _name: &'data str,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_memory_export(
        &mut self,
        _memory_index: MemoryIndex,
        _name: &'data str,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_global_export(
        &mut self,
        _global_index: GlobalIndex,
        _name: &'data str,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_start_func(&mut self, index: FuncIndex) -> WasmResult<()> {
        self.start_func = Some(index);
        Ok(())
    }

    fn declare_table_elements(
        &mut self,
        _table_index: TableIndex,
        _base: Option<GlobalIndex>,
        _offset: usize,
        _elements: Box<[FuncIndex]>,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn define_function_body(
        &mut self,
        _module_translation_state: &ModuleTranslationState,
        body_bytes: &'data [u8],
        body_offset: usize,
    ) -> Result<(), WasmError> {
        self.func_bodies.push(FunctionBody {
            body: body_bytes,
            offset: body_offset,
        });
        Ok(())
    }

    fn declare_data_initialization(
        &mut self,
        _memory_index: MemoryIndex,
        _base: Option<GlobalIndex>,
        _offset: usize,
        _data: &'data [u8],
    ) -> WasmResult<()> {
        unimplemented!()
    }
}
