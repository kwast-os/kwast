//! WASM runtime
//! Used https://github.com/bytecodealliance/wasmtime as a reference, code mostly from there.
//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

mod func_env;
pub mod main;
mod module_env;
mod reloc_sink;
mod runtime;
mod table;
pub mod vmctx;
