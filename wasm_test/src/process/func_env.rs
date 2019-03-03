use cranelift_codegen::cursor::FuncCursor;
use cranelift_codegen::ir::{ExternalName, ExtFuncData, FuncRef, Function, GlobalValueData, Heap, HeapData, HeapStyle, Inst, SigRef, Table, types, Value};
use cranelift_codegen::isa::TargetFrontendConfig;
use cranelift_wasm::{FuncIndex, GlobalIndex, GlobalVariable, MemoryIndex, ModuleEnvironment, SignatureIndex, TableIndex, WasmError};

use crate::process::module_env::ModuleEnv;

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

impl<'m, 'data> cranelift_wasm::FuncEnvironment for FuncEnv<'m, 'data> {
    fn target_config(&self) -> TargetFrontendConfig {
        self.module_env.target_config()
    }

    fn make_global(&mut self, func: &mut Function, index: GlobalIndex) -> GlobalVariable {
        unimplemented!()
    }

    fn make_heap(&mut self, func: &mut Function, index: MemoryIndex) -> Heap {
        let mem = self.module_env.get_mem(index);
        println!("{:?}", mem);

        unimplemented!()
    }

    fn make_table(&mut self, func: &mut Function, index: TableIndex) -> Table {
        unimplemented!()
    }

    fn make_indirect_sig(&mut self, func: &mut Function, index: SignatureIndex) -> SigRef {
        unimplemented!()
    }

    fn make_direct_func(&mut self, func: &mut Function, index: FuncIndex) -> FuncRef {
        // User-defined external name. Namespace doesn't matter, index is just the function index.
        let name = ExternalName::user(0, index.as_u32());
        // We got the signature earlier, get it.
        let signature = func.import_signature(self.module_env.get_sig(index));

        func.import_function(ExtFuncData {
            name,
            signature,
            colocated: true,
        })
    }

    fn translate_call_indirect(&mut self, pos: FuncCursor, table_index: TableIndex, table: Table, sig_index: SignatureIndex, sig_ref: SigRef, callee: Value, call_args: &[Value]) -> Result<Inst, WasmError> {
        unimplemented!()
    }

    fn translate_memory_grow(&mut self, pos: FuncCursor, index: MemoryIndex, heap: Heap, val: Value) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_memory_size(&mut self, pos: FuncCursor, index: MemoryIndex, heap: Heap) -> Result<Value, WasmError> {
        println!("{:?} {:?} {:?}", pos.func, index, heap);
        unimplemented!()
    }
}
