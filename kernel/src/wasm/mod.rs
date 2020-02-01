//! WASM runtime
//! Used https://github.com/bytecodealliance/wasmtime as a reference, code mostly from there.
//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

pub mod module_env;
pub mod func_env;
pub mod reloc_sink;
pub mod main;
