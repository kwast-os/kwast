use cranelift_codegen::cursor::FuncCursor;
use cranelift_codegen::ir::{ExternalName, ExtFuncData, FuncRef, Function, Heap, Inst, SigRef, Table, Value};
use cranelift_wasm::{FuncIndex, GlobalIndex, GlobalVariable, MemoryIndex, SignatureIndex, TableIndex, WasmError, FuncEnvironment, TargetEnvironment, WasmResult};

use crate::process::module_env::ModuleEnv;
use cranelift_codegen::isa::TargetFrontendConfig;

// TODO: private?

/// Used to handle transformations on functions.
pub struct FuncEnv<'m, 'data> {
    module_env: &'m ModuleEnv<'data>,
}

impl<'m, 'data> FuncEnv<'m, 'data> {
    pub fn new(module_environment: &'m ModuleEnv<'data>) -> Self {
        Self { module_env: module_environment }
    }
}

impl<'m, 'data> TargetEnvironment for FuncEnv<'m, 'data> {
    fn target_config(&self) -> TargetFrontendConfig {
        self.module_env.target_config()
    }
}

impl<'m, 'data> FuncEnvironment for FuncEnv<'m, 'data> {
    fn make_global(&mut self, _func: &mut Function, _index: GlobalIndex) -> WasmResult<GlobalVariable> {
        unimplemented!()
    }

    fn make_heap(&mut self, _func: &mut Function, index: MemoryIndex) -> WasmResult<Heap> {
        let mem = self.module_env.get_mem(index);
        println!("{:?}", mem);

        unimplemented!()
    }

    fn make_table(&mut self, _func: &mut Function, _index: TableIndex) -> WasmResult<Table> {
        unimplemented!()
    }

    fn make_indirect_sig(&mut self, _func: &mut Function, _index: SignatureIndex) -> WasmResult<SigRef> {
        unimplemented!()
    }

    fn make_direct_func(&mut self, func: &mut Function, index: FuncIndex) -> WasmResult<FuncRef> {
        // User-defined external name. Namespace doesn't matter, index is just the function index.
        let name = ExternalName::user(0, index.as_u32());
        // We got the signature earlier, get it.
        let signature = func.import_signature(self.module_env.get_sig(index));

        Ok(func.import_function(ExtFuncData {
            name,
            signature,
            colocated: true,
        }))
    }

    fn translate_call_indirect(&mut self, _pos: FuncCursor, _table_index: TableIndex, _table: Table, _sig_index: SignatureIndex, _sig_ref: SigRef, _callee: Value, _call_args: &[Value]) -> Result<Inst, WasmError> {
        unimplemented!()
    }

    fn translate_memory_grow(&mut self, _pos: FuncCursor, _index: MemoryIndex, _heap: Heap, _val: Value) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_memory_size(&mut self, pos: FuncCursor, index: MemoryIndex, heap: Heap) -> Result<Value, WasmError> {
        println!("{:?} {:?} {:?}", pos.func, index, heap);
        unimplemented!()
    }

    fn translate_memory_copy(&mut self, _pos: FuncCursor, _index: MemoryIndex, _heap: Heap, _dst: Value, _src: Value, _len: Value) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_memory_fill(&mut self, _pos: FuncCursor, _index: MemoryIndex, _heap: Heap, _dst: Value, _val: Value, _len: Value) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_memory_init(&mut self, _pos: FuncCursor, _index: MemoryIndex, _heap: Heap, _seg_index: u32, _dst: Value, _src: Value, _len: Value) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_data_drop(&mut self, _pos: FuncCursor, _seg_index: u32) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_table_size(&mut self, _pos: FuncCursor, _index: TableIndex, _table: Table) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_table_copy(&mut self, _pos: FuncCursor, _dst_table_index: TableIndex, _dst_table: Table, _src_table_index: TableIndex, _src_table: Table, _dst: Value, _src: Value, _len: Value) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_table_init(&mut self, _pos: FuncCursor, _seg_index: u32, _table_index: TableIndex, _table: Table, _dst: Value, _src: Value, _len: Value) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_elem_drop(&mut self, _pos: FuncCursor, _seg_index: u32) -> Result<(), WasmError> {
        unimplemented!()
    }
}
