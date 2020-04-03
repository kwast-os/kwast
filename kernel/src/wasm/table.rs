use crate::arch::address::VirtAddr;
use crate::wasm::vmctx::{VmTable, VmTableElement};
use alloc::vec::Vec;
use cranelift_wasm::TableElementType;

/// A table, manages table data for the runtime.
pub struct Table {
    vec: Vec<VmTableElement>,
}

impl Table {
    /// Creates a new table.
    pub fn new(table: &cranelift_wasm::Table) -> Self {
        let vec = match table.ty {
            TableElementType::Func => vec![VmTableElement::null(); table.minimum as usize],
            TableElementType::Val(_) => unimplemented!("other type than anyfunc"),
        };

        Self { vec }
    }

    /// Sets a table element.
    pub fn set(&mut self, offset: usize, value: VmTableElement) {
        self.vec[offset] = value;
    }

    /// Gets the VmContext representation
    pub fn as_vm_table(&self) -> VmTable {
        VmTable {
            base_address: VirtAddr::new(self.vec.as_ptr() as usize),
            amount_items: self.vec.len() as u32,
        }
    }
}
