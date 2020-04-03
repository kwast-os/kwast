use cranelift_codegen::ir::{types, AbiParam, ArgumentPurpose, Signature};
use cranelift_codegen::isa::TargetFrontendConfig;

/// Runtime namespace for `ExternalName`.
pub const RUNTIME_NAMESPACE: u32 = 1;
pub const RUNTIME_MEMORY_GROW_IDX: u32 = 0;
pub const RUNTIME_MEMORY_SIZE_IDX: u32 = 1;

/// Runtime function data.
pub struct RuntimeFunctionData {
    pub index: u32,
    pub signature: Signature,
}

/// Runtime functions container.
pub struct RuntimeFunctions {
    pub memory_grow: RuntimeFunctionData,
    pub memory_size: RuntimeFunctionData,
}

// TODO: lazy static this instead of per module
impl RuntimeFunctions {
    /// Creates a new instance of the runtime function data.
    pub fn new(cfg: TargetFrontendConfig) -> Self {
        let memory_grow = RuntimeFunctionData {
            index: RUNTIME_MEMORY_GROW_IDX,
            signature: Signature {
                params: vec![
                    AbiParam::special(cfg.pointer_type(), ArgumentPurpose::VMContext),
                    AbiParam::new(types::I32), // Memory index
                    AbiParam::new(types::I32), // Pages
                ],
                returns: vec![AbiParam::new(types::I32)],
                call_conv: cfg.default_call_conv,
            },
        };

        let memory_size = RuntimeFunctionData {
            index: RUNTIME_MEMORY_SIZE_IDX,
            signature: Signature {
                params: vec![
                    AbiParam::special(cfg.pointer_type(), ArgumentPurpose::VMContext),
                    AbiParam::new(types::I32), // Memory index
                ],
                returns: vec![AbiParam::new(types::I32)],
                call_conv: cfg.default_call_conv,
            },
        };

        Self {
            memory_grow,
            memory_size,
        }
    }
}
