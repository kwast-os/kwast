//! WASM runtime
//! Used https://github.com/bytecodealliance/wasmtime as a reference, code mostly from there.
//! Based on https://github.com/bytecodealliance/wasmtime/tree/master/crates/jit/src

mod module_env;
mod func_env;
mod reloc_sink;
pub mod main;
