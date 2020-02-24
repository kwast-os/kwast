//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use alloc::vec::Vec;
use cranelift_codegen::binemit::{self, Reloc};
use cranelift_codegen::ir::{ExternalName, JumpTable, LibCall};
use cranelift_wasm::FuncIndex;

/// Relocation target.
#[derive(Debug)]
pub enum RelocationTarget {
    /// Relocation is for a user-defined function.
    UserFunction(FuncIndex),
    /// Relocation is for a lib-defined function.
    LibCall(LibCall),
    /// Relocation is for a jump table.
    JumpTable(JumpTable),
}

/// A relocation entry for the function.
#[derive(Debug)]
pub struct Relocation {
    pub code_offset: u32,
    pub reloc: Reloc,
    pub target: RelocationTarget,
    pub addend: i64,
}

/// Relocation sink, stores relocations for code.
#[derive(Debug)]
pub struct RelocSink {
    pub relocations: Vec<Relocation>,
}

impl RelocSink {
    pub fn new() -> Self {
        Self {
            relocations: Vec::new(),
        }
    }
}

impl binemit::RelocSink for RelocSink {
    fn reloc_block(&mut self, _: u32, _: Reloc, _: u32) {
        unimplemented!()
    }

    fn reloc_external(&mut self, code_offset: u32, reloc: Reloc, name: &ExternalName, addend: i64) {
        let reloc_type = if let ExternalName::User { namespace, index } = *name {
            debug_assert_eq!(namespace, 0);
            RelocationTarget::UserFunction(FuncIndex::from_u32(index))
        } else if let ExternalName::LibCall(libcall) = *name {
            RelocationTarget::LibCall(libcall)
        } else {
            panic!("unknown relocation type")
        };

        self.relocations.push(Relocation {
            code_offset,
            reloc,
            target: reloc_type,
            addend,
        });
    }

    fn reloc_constant(&mut self, _: u32, _: Reloc, _: u32) {
        // Not necessary unless we split the code and rodata.
    }

    fn reloc_jt(&mut self, _code_offset: u32, _reloc: Reloc, _jt: JumpTable) {
        /*self.relocations.push(Relocation {
            code_offset,
            reloc,
            target: RelocationTarget::JumpTable(jt),
            addend: 0,
        });*/
        // TODO: investigate: is this necessary?
        unimplemented!()
    }
}
