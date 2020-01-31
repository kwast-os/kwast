use cranelift_codegen::ir::Signature;
use cranelift_codegen::isa;
use cranelift_wasm::{FuncIndex, Global, GlobalIndex, Memory, MemoryIndex, SignatureIndex, Table, TableIndex, WasmError, TargetEnvironment, ModuleEnvironment, WasmResult, ModuleTranslationState};
use cranelift_codegen::isa::TargetFrontendConfig;

// TODO: make things private again

pub struct ModuleEnv<'data> {
    /// Passed target configuration.
    cfg: isa::TargetFrontendConfig,
    /// Starting function.
    start_func: Option<FuncIndex>,
    /// Vector of all signatures.
    pub signatures: Vec<Signature>,
    /// Function types.
    func_types: Vec<SignatureIndex>,
    /// Function Wasm body contents.
    pub func_bodies: Vec<&'data [u8]>,
    /// Memories.
    memories: Vec<Memory>,
}

impl<'data> ModuleEnv<'data> {
    pub fn new(cfg: TargetFrontendConfig) -> Self {
        Self {
            cfg,
            start_func: None,
            signatures: Vec::new(),
            func_types: Vec::new(),
            func_bodies: Vec::new(),
            memories: Vec::new(),
        }
    }

    pub fn get_sig(&self, index: FuncIndex) -> Signature {
        let sig_index = self.func_types[index.as_u32() as usize];
        self.signatures[sig_index.as_u32() as usize].clone()
    }

    pub fn get_mem(&self, index: MemoryIndex) -> Memory {
        // TODO: difference between imported/defined memory

        self.memories[index.as_u32() as usize]
    }
}

impl<'data> TargetEnvironment for ModuleEnv<'data> {
    fn target_config(&self) -> TargetFrontendConfig {
        self.cfg
    }
}

impl<'data> ModuleEnvironment<'data> for ModuleEnv<'data> {
    fn declare_signature(&mut self, sig: Signature) -> WasmResult<()> {
        self.signatures.push(sig);
        Ok(())
    }

    fn declare_func_import(
        &mut self,
        _sig_index: SignatureIndex,
        _module: &'data str,
        _field: &'data str,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_table_import(&mut self, _table: Table, _module: &'data str, _field: &'data str) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_memory_import(&mut self, _memory: Memory, _module: &'data str, _field: &'data str) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_global_import(&mut self, _global: Global, _module: &'data str, _field: &'data str) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_func_type(&mut self, sig_index: SignatureIndex) -> WasmResult<()> {
        self.func_types.push(sig_index);
        Ok(())
    }

    fn declare_table(&mut self, _table: Table) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_memory(&mut self, memory: Memory) -> WasmResult<()> {
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

    fn declare_table_export(&mut self, _table_index: TableIndex, _name: &'data str) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_memory_export(&mut self, _memory_index: MemoryIndex, _name: &'data str) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_global_export(&mut self, _global_index: GlobalIndex, _name: &'data str) -> WasmResult<()> {
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
        _body_offset: usize, // TODO
    ) -> Result<(), WasmError> {
        // We could also compile now, but that means we need to keep the IR representation in memory
        // until we can use them. Storing the function bodies reference is cheaper now because the
        // WASM file is already in memory.
        self.func_bodies.push(body_bytes);
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
