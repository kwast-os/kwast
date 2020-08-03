//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use cranelift_codegen::ir::{AbiParam, ArgumentPurpose, Signature};
use cranelift_codegen::isa;
use cranelift_codegen::isa::TargetFrontendConfig;
use cranelift_wasm::{
    DataIndex, ElemIndex, FuncIndex, Global, GlobalIndex, Memory, MemoryIndex, ModuleEnvironment,
    ModuleTranslationState, SignatureIndex, Table, TableIndex, TargetEnvironment, WasmError,
    WasmFuncType, WasmResult,
};
use hashbrown::HashMap;

pub struct FunctionBody<'data> {
    pub body: &'data [u8],
    pub offset: usize,
}

/// Export.
pub enum Export {
    /// Function export.
    Function(FuncIndex),
}

/// Function import.
#[derive(Debug)]
pub struct FunctionImport {
    pub module: String,
    pub field: String,
}

/// Table elements.
pub struct TableElements {
    /// Index of the table where these elements belong to.
    pub index: TableIndex,
    /// Base offset by a global into the table.
    pub base: Option<GlobalIndex>,
    /// Base offset by a number into the table.
    pub offset: usize,
    /// The table elements.
    pub elements: Box<[FuncIndex]>,
}

/// Memory data initializer.
#[derive(Debug)]
pub struct DataInitializer<'a> {
    pub memory_index: MemoryIndex,
    pub base: Option<GlobalIndex>,
    pub offset: usize,
    pub data: &'a [u8],
}

/// The module environment.
pub struct ModuleEnv<'data> {
    /// Passed target configuration.
    cfg: isa::TargetFrontendConfig,
    /// Starting function.
    pub start_func: Option<FuncIndex>,
    /// Vector of all signatures.
    pub signatures: Vec<Signature>,
    /// Function signatures.
    pub func_sigs: Vec<SignatureIndex>,
    /// Function Wasm body contents.
    pub func_bodies: Vec<FunctionBody<'data>>,
    /// Memories.
    pub memories: Vec<Memory>,
    /// Keep track of the imported functions.
    pub function_imports: Vec<FunctionImport>,
    /// Tables.
    pub tables: Vec<Table>,
    /// Table elements.
    pub table_elements: Vec<TableElements>,
    /// Globals.
    pub globals: Vec<Global>,
    /// Data initializers.
    pub data_initializers: Vec<DataInitializer<'data>>,
    /// All exports.
    pub exports: HashMap<&'data str, Export>,
}

impl<'data> ModuleEnv<'data> {
    /// Creates a new module environment for the given configuration.
    pub fn new(cfg: TargetFrontendConfig) -> Self {
        Self {
            cfg,
            start_func: None,
            signatures: Vec::new(),
            func_sigs: Vec::new(),
            func_bodies: Vec::new(),
            memories: Vec::new(),
            function_imports: Vec::new(),
            tables: Vec::new(),
            table_elements: Vec::new(),
            globals: Vec::new(),
            data_initializers: Vec::new(),
            exports: HashMap::new(),
        }
    }

    /// Gets the signature of the given function.
    pub fn get_sig_from_func(&self, index: FuncIndex) -> Signature {
        let sig_index = self.func_sigs[index.as_u32() as usize];
        self.signatures[sig_index.as_u32() as usize].clone()
    }

    /// Gets the signature.
    pub fn get_sig_from_sigidx(&self, sig_idx: SignatureIndex) -> Signature {
        self.signatures[sig_idx.as_u32() as usize].clone()
    }

    /// Returns whether the function index corresponds to an imported function.
    pub fn is_imported_func(&self, index: FuncIndex) -> bool {
        // Imported functions are defined first.
        (index.as_u32() as usize) < self.function_imports.len()
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

    fn declare_signature(
        &mut self,
        _wasm_func_type: WasmFuncType,
        mut sig: Signature,
    ) -> WasmResult<()> {
        sig.params.insert(
            0,
            AbiParam::special(self.pointer_type(), ArgumentPurpose::VMContext),
        );
        self.signatures.push(sig);
        Ok(())
    }

    fn declare_func_import(
        &mut self,
        sig_index: SignatureIndex,
        module: &'data str,
        field: &'data str,
    ) -> WasmResult<()> {
        self.func_sigs.push(sig_index);
        self.function_imports.push(FunctionImport {
            module: String::from(module),
            field: String::from(field),
        });
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
        self.func_sigs.reserve_exact(num as usize);
        Ok(())
    }

    fn declare_func_type(&mut self, sig_index: SignatureIndex) -> WasmResult<()> {
        self.func_sigs.push(sig_index);
        Ok(())
    }

    fn reserve_tables(&mut self, num: u32) -> WasmResult<()> {
        self.tables.reserve_exact(num as usize);
        Ok(())
    }

    fn declare_table(&mut self, table: Table) -> WasmResult<()> {
        self.tables.push(table);
        Ok(())
    }

    fn reserve_memories(&mut self, num: u32) -> WasmResult<()> {
        self.memories.reserve_exact(num as usize);
        Ok(())
    }

    fn declare_memory(&mut self, memory: Memory) -> WasmResult<()> {
        // TODO: Shared memories and more than one memory not supported right now.
        assert_eq!(memory.shared, false);
        assert_eq!(self.memories.len(), 0);
        self.memories.push(memory);
        Ok(())
    }

    fn reserve_globals(&mut self, num: u32) -> WasmResult<()> {
        self.globals.reserve_exact(num as usize);
        Ok(())
    }

    fn declare_global(&mut self, global: Global) -> WasmResult<()> {
        self.globals.push(global);
        Ok(())
    }

    fn declare_func_export(&mut self, func_index: FuncIndex, name: &'data str) -> WasmResult<()> {
        self.exports.insert(name, Export::Function(func_index));
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
        //unimplemented!()
        // TODO
        Ok(())
    }

    fn declare_global_export(
        &mut self,
        _global_index: GlobalIndex,
        _name: &'data str,
    ) -> WasmResult<()> {
        //unimplemented!()
        // TODO
        Ok(())
    }

    fn declare_start_func(&mut self, index: FuncIndex) -> WasmResult<()> {
        self.start_func = Some(index);
        Ok(())
    }

    fn reserve_table_elements(&mut self, num: u32) -> WasmResult<()> {
        self.table_elements.reserve_exact(num as usize);
        Ok(())
    }

    fn declare_table_elements(
        &mut self,
        table_index: TableIndex,
        base: Option<GlobalIndex>,
        offset: usize,
        elements: Box<[FuncIndex]>,
    ) -> WasmResult<()> {
        self.table_elements.push(TableElements {
            index: table_index,
            base,
            offset,
            elements,
        });
        Ok(())
    }

    fn declare_passive_element(
        &mut self,
        _index: ElemIndex,
        _elements: Box<[FuncIndex]>,
    ) -> WasmResult<()> {
        unimplemented!()
    }

    fn declare_passive_data(
        &mut self,
        _data_index: DataIndex,
        _data: &'data [u8],
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

    fn reserve_data_initializers(&mut self, num: u32) -> WasmResult<()> {
        self.data_initializers.reserve_exact(num as usize);
        Ok(())
    }

    fn declare_data_initialization(
        &mut self,
        memory_index: MemoryIndex,
        base: Option<GlobalIndex>,
        offset: usize,
        data: &'data [u8],
    ) -> WasmResult<()> {
        self.data_initializers.push(DataInitializer {
            memory_index,
            base,
            offset,
            data,
        });
        Ok(())
    }
}
