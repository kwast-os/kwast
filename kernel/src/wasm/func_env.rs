//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use crate::wasm::module_env::ModuleEnv;
use crate::wasm::vmctx::{HEAP_SIZE, HEAP_VMCTX_OFF};
use alloc::vec::Vec;
use cranelift_codegen::cursor::FuncCursor;
use cranelift_codegen::ir::immediates::Offset32;
use cranelift_codegen::ir::InstBuilder;
use cranelift_codegen::ir::{
    types, ArgumentPurpose, ExtFuncData, ExternalName, FuncRef, Function, GlobalValue,
    GlobalValueData, Heap, HeapData, HeapStyle, Inst, SigRef, Table, Value,
};
use cranelift_codegen::isa::TargetFrontendConfig;
use cranelift_wasm::{
    FuncEnvironment, FuncIndex, GlobalIndex, GlobalVariable, MemoryIndex, SignatureIndex,
    TableIndex, TargetEnvironment, WasmError, WasmResult,
};

/// Used to handle transformations on functions.
pub struct FuncEnv<'m, 'data> {
    module_env: &'m ModuleEnv<'data>,
    vmctx: Option<GlobalValue>,
    heap_base: Option<GlobalValue>,
}

impl<'m, 'data> FuncEnv<'m, 'data> {
    pub fn new(module_environment: &'m ModuleEnv<'data>) -> Self {
        Self {
            module_env: module_environment,
            vmctx: None,
            heap_base: None,
        }
    }

    fn vmctx(&mut self, func: &mut Function) -> GlobalValue {
        self.vmctx.unwrap_or_else(|| {
            let vmctx = func.create_global_value(GlobalValueData::VMContext);
            self.vmctx = Some(vmctx);
            vmctx
        })
    }
}

impl<'m, 'data> TargetEnvironment for FuncEnv<'m, 'data> {
    fn target_config(&self) -> TargetFrontendConfig {
        self.module_env.target_config()
    }
}

impl<'m, 'data> FuncEnvironment for FuncEnv<'m, 'data> {
    fn make_global(
        &mut self,
        _func: &mut Function,
        _index: GlobalIndex,
    ) -> WasmResult<GlobalVariable> {
        unimplemented!()
    }

    fn make_heap(&mut self, func: &mut Function, index: MemoryIndex) -> WasmResult<Heap> {
        assert_eq!(index.as_u32(), 0);

        let heap_base = self.heap_base.unwrap_or_else(|| {
            let vmctx = self.vmctx(func);
            let heap_base = func.create_global_value(GlobalValueData::Load {
                base: vmctx,
                offset: Offset32::new(HEAP_VMCTX_OFF),
                global_type: self.pointer_type(),
                readonly: true,
            });
            self.heap_base = Some(heap_base);
            heap_base
        });

        Ok(func.create_heap(HeapData {
            base: heap_base,
            min_size: HEAP_SIZE.into(),
            offset_guard_size: 0.into(),
            style: HeapStyle::Static {
                bound: HEAP_SIZE.into(),
            },
            index_type: types::I32,
        }))
    }

    fn make_table(&mut self, _func: &mut Function, _index: TableIndex) -> Result<Table, WasmError> {
        unimplemented!()
    }

    fn make_indirect_sig(
        &mut self,
        _func: &mut Function,
        _index: SignatureIndex,
    ) -> WasmResult<SigRef> {
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

    fn translate_call_indirect(
        &mut self,
        _pos: FuncCursor,
        _table_index: TableIndex,
        _table: Table,
        _sig_index: SignatureIndex,
        _sig_ref: SigRef,
        _callee: Value,
        _call_args: &[Value],
    ) -> Result<Inst, WasmError> {
        unimplemented!()
    }

    fn translate_call(
        &mut self,
        mut pos: FuncCursor,
        _callee_index: FuncIndex,
        callee: FuncRef,
        call_args: &[Value],
    ) -> WasmResult<Inst> {
        let vmctx = pos.func.special_param(ArgumentPurpose::VMContext).unwrap();

        let mut call_args_with_vmctx = Vec::with_capacity(call_args.len() + 1);
        call_args_with_vmctx.extend_from_slice(call_args);
        call_args_with_vmctx.push(vmctx);

        Ok(pos.ins().call(callee, &call_args_with_vmctx))
    }

    fn translate_memory_grow(
        &mut self,
        _pos: FuncCursor,
        _index: MemoryIndex,
        _heap: Heap,
        _val: Value,
    ) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_memory_size(
        &mut self,
        pos: FuncCursor,
        index: MemoryIndex,
        heap: Heap,
    ) -> Result<Value, WasmError> {
        println!("{:?} {:?} {:?}", pos.func, index, heap);
        unimplemented!()
    }

    fn translate_memory_copy(
        &mut self,
        _pos: FuncCursor,
        _index: MemoryIndex,
        _heap: Heap,
        _dst: Value,
        _src: Value,
        _len: Value,
    ) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_memory_fill(
        &mut self,
        _pos: FuncCursor,
        _index: MemoryIndex,
        _heap: Heap,
        _dst: Value,
        _val: Value,
        _len: Value,
    ) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_memory_init(
        &mut self,
        _pos: FuncCursor,
        _index: MemoryIndex,
        _heap: Heap,
        _seg_index: u32,
        _dst: Value,
        _src: Value,
        _len: Value,
    ) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_data_drop(&mut self, _pos: FuncCursor, _seg_index: u32) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_table_size(
        &mut self,
        _pos: FuncCursor,
        _index: TableIndex,
        _table: Table,
    ) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_table_grow(
        &mut self,
        _pos: FuncCursor,
        _table_index: u32,
        _delta: Value,
        _init_value: Value,
    ) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_table_get(
        &mut self,
        _pos: FuncCursor,
        _table_index: u32,
        _index: Value,
    ) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_table_set(
        &mut self,
        _pos: FuncCursor,
        _table_index: u32,
        _value: Value,
        _index: Value,
    ) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_table_copy(
        &mut self,
        _pos: FuncCursor,
        _dst_table_index: TableIndex,
        _dst_table: Table,
        _src_table_index: TableIndex,
        _src_table: Table,
        _dst: Value,
        _src: Value,
        _len: Value,
    ) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_table_fill(
        &mut self,
        _pos: FuncCursor,
        _table_index: u32,
        _dst: Value,
        _val: Value,
        _len: Value,
    ) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_table_init(
        &mut self,
        _pos: FuncCursor,
        _seg_index: u32,
        _table_index: TableIndex,
        _table: Table,
        _dst: Value,
        _src: Value,
        _len: Value,
    ) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_elem_drop(&mut self, _pos: FuncCursor, _seg_index: u32) -> Result<(), WasmError> {
        unimplemented!()
    }

    fn translate_ref_func(
        &mut self,
        _pos: FuncCursor,
        _func_index: u32,
    ) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_custom_global_get(
        &mut self,
        _pos: FuncCursor,
        _global_index: GlobalIndex,
    ) -> Result<Value, WasmError> {
        unimplemented!()
    }

    fn translate_custom_global_set(
        &mut self,
        _pos: FuncCursor,
        _global_index: GlobalIndex,
        _val: Value,
    ) -> Result<(), WasmError> {
        unimplemented!()
    }
}
