use crate::arch::address::VirtAddr;
use crate::wasm::vmctx::VmContext;
use hashbrown::HashMap;
use lazy_static::lazy_static;

#[repr(u16)]
pub enum Errno {
    Success,
    // TODO
}

type WasmPtr<T> = T; // TODO

type WasmStatus = Result<(), Errno>;

abi_functions! {
    first: (a: i32, b: i64) -> Errno,
}

impl AbiFunctions for VmContext {
    fn first(&self, a: i32, b: i64) -> WasmStatus {
        Ok(())
    }
}

pub fn get_address_for_wasi(name: &str) -> Option<VirtAddr> {
    ABI_MAP.get(name).map(|e| VirtAddr::new(*e))
}
