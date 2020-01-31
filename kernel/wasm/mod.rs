//! WASM runtime
//! Used https://github.com/bytecodealliance/wasmtime as a reference, code mostly from there.

pub mod module_env;
pub mod func_env;
pub mod reloc_sink;
