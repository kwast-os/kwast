use std::fs::File;
use std::io::prelude::*;

use cranelift_codegen::binemit::{NullTrapSink, NullStackmapSink};
use cranelift_codegen::Context;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_native;
use cranelift_wasm::FuncTranslator;
use cranelift_wasm::translate_module;

use process::func_env::FuncEnv;
use process::module_env::ModuleEnv;
use process::reloc_sink::RelocSink;

mod process;

fn main() {
    // Test
    let mut f = File::open("samples/sum.wasm").expect("Could not open file");
    let mut buffer = [0; 65]; // hardcoded
    f.read(&mut buffer).expect("Could not read file");

    let isa_builder = cranelift_native::builder().unwrap();
    let mut flag_builder = settings::builder();

    // Flags
    flag_builder.set("opt_level", "speed_and_size").unwrap();

    let flags = settings::Flags::new(flag_builder);
    let isa = isa_builder.finish(flags);

    // Translate
    let mut env = ModuleEnv::new(isa.frontend_config());
    let mut translation = translate_module(&buffer, &mut env).unwrap();

    for i in 0..=0 {
        //
        let mut mem: Vec<u8> = Vec::new();
        let mut ctx = Context::new(); // TODO: compile_and_emit -> emit_to_memory
        let mut reloc_sink = RelocSink::new();
        let mut trap_sink = NullTrapSink {};
        let mut null_stackmap_sink = NullStackmapSink {};

        ctx.func.signature = env.signatures[i].clone(); // TODO: idx, clone?

        let mut func_trans = FuncTranslator::new();
        func_trans
            .translate(
                &mut translation,
                &env.func_bodies[i],
                0,
                &mut ctx.func,
                &mut FuncEnv::new(&env),
            )
            .unwrap();

        ctx.compile_and_emit(&*isa, &mut mem, &mut reloc_sink, &mut trap_sink, &mut null_stackmap_sink)
            .unwrap();

        println!("-----------------");
        println!("-----------------");
        println!("-----------------");
        println!("survived: {:?}", mem);
        println!("{:?}", ctx.func);

        let mut of = File::create("output").expect("Unable to create output file");
        of.write(&mem).unwrap();
    }
}
