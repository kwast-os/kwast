use crate::wasm::main::{WASM_CALL_CONV, WASM_VMCTX_TYPE};
use cranelift_codegen::ir::{types, AbiParam, ArgumentPurpose, Signature};
use lazy_static::lazy_static;

/// Runtime namespace for `ExternalName`.
pub const RUNTIME_NAMESPACE: u32 = 1;
pub const RUNTIME_MEMORY_GROW_IDX: u32 = 0;
pub const RUNTIME_MEMORY_SIZE_IDX: u32 = 1;

/// Runtime function data.
pub struct RuntimeFunctionData {
    pub index: u32,
    pub signature: Signature,
}

lazy_static! {
    pub static ref RUNTIME_MEMORY_GROW_DATA: RuntimeFunctionData = RuntimeFunctionData {
        index: RUNTIME_MEMORY_GROW_IDX,
        signature: Signature {
            params: vec![
                AbiParam::special(WASM_VMCTX_TYPE, ArgumentPurpose::VMContext),
                AbiParam::new(types::I32), // Memory index
                AbiParam::new(types::I32), // Pages
            ],
            returns: vec![AbiParam::new(types::I32)],
            call_conv: WASM_CALL_CONV,
        },
    };

    pub static ref RUNTIME_MEMORY_SIZE_DATA: RuntimeFunctionData = RuntimeFunctionData {
        index: RUNTIME_MEMORY_SIZE_IDX,
        signature: Signature {
            params: vec![
                AbiParam::special(WASM_VMCTX_TYPE, ArgumentPurpose::VMContext),
                AbiParam::new(types::I32), // Memory index
            ],
            returns: vec![AbiParam::new(types::I32)],
            call_conv: WASM_CALL_CONV,
        },
    };
}
