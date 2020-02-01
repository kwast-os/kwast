//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use cranelift_codegen::binemit::{self, Reloc};
use cranelift_codegen::ir::{ExternalName, JumpTable, LibCall};
use alloc::vec::Vec;
use cranelift_wasm::FuncIndex;

// TODO: make private?

/// Relocation type.
#[derive(Debug)]
enum RelocationType {
    /// Relocation is for a user-defined function.
    UserFunction(FuncIndex),
    /// Relocation is for a lib-defined function.
    LibCall(LibCall),
    /// Relocation is for a jump table.
    JumpTable(JumpTable),
}

/// A relocation entry for the function.
#[derive(Debug)]
struct Relocation {
    code_offset: u32,
    reloc: Reloc,
    reloc_type: RelocationType,
    addend: i64,
}

/// Relocation sink, stores relocations for code.
#[derive(Debug)]
pub struct RelocSink {
    relocations: Vec<Relocation>,
}

impl RelocSink {
    pub fn new() -> Self {
        Self {
            relocations: Vec::new(),
        }
    }
}

impl binemit::RelocSink for RelocSink {
    fn reloc_ebb(&mut self, _: u32, _: Reloc, _: u32) {
        unimplemented!()
    }

    fn reloc_external(&mut self, code_offset: u32, reloc: Reloc, name: &ExternalName, addend: i64) {
        let reloc_type = if let ExternalName::User { index, .. } = *name {
            RelocationType::UserFunction(FuncIndex::from_u32(index))
        } else if let ExternalName::LibCall(libcall) = *name {
            RelocationType::LibCall(libcall)
        } else {
            panic!("unknown relocation type")
        };

        self.relocations.push(Relocation {
            code_offset,
            reloc,
            reloc_type,
            addend,
        });
    }

    fn reloc_constant(&mut self, _: u32, _: Reloc, _: u32) {
        // Do nothing.
    }

    fn reloc_jt(&mut self, code_offset: u32, reloc: Reloc, jt: JumpTable) {
        self.relocations.push(Relocation {
            code_offset,
            reloc,
            reloc_type: RelocationType::JumpTable(jt),
            addend: 0,
        });
    }
}
