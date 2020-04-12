//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

use crate::wasm::runtime::RUNTIME_NAMESPACE;
use alloc::vec::Vec;
use cranelift_codegen::binemit::{self, Addend, CodeOffset, Reloc};
use cranelift_codegen::ir::{ExternalName, JumpTable, LibCall, SourceLoc};
use cranelift_wasm::FuncIndex;

/// Relocation target.
#[derive(Debug)]
pub enum RelocationTarget {
    /// Relocation is for a user-defined function.
    UserFunction(FuncIndex),
    /// Runtime function.
    RuntimeFunction(u32),
    /// Relocation is for a lib-defined function.
    LibCall(LibCall),
}

/// A relocation entry for the function.
#[derive(Debug)]
pub struct Relocation {
    pub code_offset: CodeOffset,
    pub reloc: Reloc,
    pub target: RelocationTarget,
    pub addend: Addend,
}

/// Relocation sink, stores relocations for code.
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

    fn reloc_external(
        &mut self,
        code_offset: CodeOffset,
        _source_loc: SourceLoc,
        reloc: Reloc,
        name: &ExternalName,
        addend: Addend,
    ) {
        let reloc_type = match *name {
            ExternalName::User {
                namespace: 0,
                index,
            } => RelocationTarget::UserFunction(FuncIndex::from_u32(index)),
            ExternalName::User {
                namespace: RUNTIME_NAMESPACE,
                index,
            } => RelocationTarget::RuntimeFunction(index),
            ExternalName::LibCall(libcall) => RelocationTarget::LibCall(libcall),
            _ => unreachable!(),
        };

        self.relocations.push(Relocation {
            code_offset,
            reloc,
            target: reloc_type,
            addend,
        });
    }

    #[inline]
    fn reloc_constant(&mut self, _: u32, _: Reloc, _: u32) {
        // Not necessary atm because our code and rodata is not split.
    }

    #[inline]
    fn reloc_jt(&mut self, _code_offset: u32, _reloc: Reloc, _jt: JumpTable) {
        // Not necessary atm because our code and rodata is not split.
        //self.relocations.push(Relocation {
        //    code_offset,
        //    reloc,
        //    target: RelocationTarget::JumpTable(jt),
        //    addend: 0,
        //});
    }
}
