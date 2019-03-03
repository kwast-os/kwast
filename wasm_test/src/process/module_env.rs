use cranelift_codegen::ir::Signature;
use cranelift_codegen::isa;
use cranelift_wasm::{
    FuncIndex, Global, GlobalIndex, Memory, MemoryIndex, SignatureIndex, Table,
    TableIndex, WasmError,
};

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
    pub fn new(cfg: isa::TargetFrontendConfig) -> Self {
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

impl<'data> cranelift_wasm::ModuleEnvironment<'data> for ModuleEnv<'data> {
    fn target_config(&self) -> isa::TargetFrontendConfig {
        self.cfg
    }

    fn declare_signature(&mut self, sig: Signature) {
        self.signatures.push(sig);
    }

    fn declare_func_import(
        &mut self,
        sig_index: SignatureIndex,
        module: &'data str,
        field: &'data str,
    ) {
        unimplemented!()
    }

    fn declare_table_import(&mut self, table: Table, module: &'data str, field: &'data str) {
        unimplemented!()
    }

    fn declare_memory_import(&mut self, memory: Memory, module: &'data str, field: &'data str) {
        unimplemented!()
    }

    fn declare_global_import(&mut self, global: Global, module: &'data str, field: &'data str) {
        unimplemented!()
    }

    fn declare_func_type(&mut self, sig_index: SignatureIndex) {
        self.func_types.push(sig_index);
    }

    fn declare_table(&mut self, table: Table) {
        unimplemented!()
    }

    fn declare_memory(&mut self, memory: Memory) {
        self.memories.push(memory);
    }

    fn declare_global(&mut self, global: Global) {
        unimplemented!()
    }

    fn declare_func_export(&mut self, func_index: FuncIndex, name: &'data str) {
        println!("declare func export: {}", name);
    }

    fn declare_table_export(&mut self, table_index: TableIndex, name: &'data str) {
        unimplemented!()
    }

    fn declare_memory_export(&mut self, memory_index: MemoryIndex, name: &'data str) {
        unimplemented!()
    }

    fn declare_global_export(&mut self, global_index: GlobalIndex, name: &'data str) {
        unimplemented!()
    }

    fn declare_start_func(&mut self, index: FuncIndex) {
        self.start_func = Some(index);
    }

    fn declare_table_elements(
        &mut self,
        table_index: TableIndex,
        base: Option<GlobalIndex>,
        offset: usize,
        elements: Box<[FuncIndex]>,
    ) {
        unimplemented!()
    }

    fn define_function_body(&mut self, body_bytes: &'data [u8]) -> Result<(), WasmError> {
        // We could also compile now, but that means we need to keep the IR representation in memory
        // until we can use them. Storing the function bodies reference is cheaper now because the
        // WASM file is already in memory.
        self.func_bodies.push(body_bytes);
        Ok(())
    }

    fn declare_data_initialization(
        &mut self,
        memory_index: MemoryIndex,
        base: Option<GlobalIndex>,
        offset: usize,
        data: &'data [u8],
    ) {
        unimplemented!()
    }
}
