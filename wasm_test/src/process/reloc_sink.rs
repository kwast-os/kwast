use cranelift_codegen::binemit::{self, Reloc};
use cranelift_codegen::ir::{ExternalName, JumpTable};

// TODO: make private?

/// Relocation sink, stores relocations for code.
pub struct RelocSink {}

impl RelocSink {
    pub fn new() -> Self {
        Self {}
    }
}

impl binemit::RelocSink for RelocSink {
    fn reloc_ebb(&mut self, _: u32, _: Reloc, _: u32) {
        unimplemented!()
    }

    fn reloc_external(&mut self, code_offset: u32, reloc: Reloc, name: &ExternalName, addend: i64) {
        //unimplemented!()
        println!("reloc_external: {:?} {:?} {:?} {:?}", code_offset, reloc, name, addend);
    }

    fn reloc_jt(&mut self, _: u32, _: Reloc, _: JumpTable) {
        unimplemented!()
    }
}
