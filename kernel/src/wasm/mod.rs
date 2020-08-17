//! WebAssembly runtime
//! Used https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src as a reference.

mod func_env;
pub mod main;
mod module_env;
mod reloc_sink;
mod runtime;
mod table;
pub mod vmctx;
pub mod wasi;
