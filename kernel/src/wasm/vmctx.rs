use crate::arch::address::VirtAddr;
use crate::arch::paging::PAGE_SIZE;
use crate::wasm::table::Table;
use alloc::alloc::{alloc, dealloc, handle_alloc_error};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::mem::{align_of, size_of};
use core::slice;
use cranelift_wasm::{Global, GlobalInit, SignatureIndex, TableIndex};

pub const WASM_PAGE_SIZE: usize = 64 * 1024;

pub const HEAP_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4 GiB

pub const HEAP_GUARD_SIZE: u64 = PAGE_SIZE as u64;

/// Table representation as it is for the VmContext.
#[repr(C)]
#[derive(Debug)]
pub struct VmTable {
    /// Base address to the function pointers.
    pub base_address: VirtAddr,
    /// Amount of items currently in.
    pub amount_items: u32,
}

/// A single element in the table representation for a VmContext.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VmTableElement {
    pub address: VirtAddr,
    pub sig_idx: u64,
}

#[repr(C)]
pub struct VmFunctionImportEntry {
    pub address: VirtAddr,
}

/// Context for a Wasm execution.
/// Layout of the VmContext:
///
/// -----------------------------
/// |       Heap pointer        |
/// -----------------------------
/// |        all globals        |
/// -----------------------------
/// | all VmFunctionImportEntry |
/// -----------------------------
/// |       all VmTable         |
/// -----------------------------
///
#[repr(C, align(16))]
pub struct VmContext {
    // Note: Variable size struct, heap pointer provided for convenience.
    pub heap_ptr: VirtAddr,
}

// All globals have the same size right now.
// TODO: make sure not all globals take the same amount of bytes
type VmGlobal = [u8; 8];

pub struct VmContextContainer {
    ptr: VirtAddr,
    num_imported_funcs: u32,
    num_globals: u32,
    tables: Vec<Table>,
}

impl VmTableElement {
    /// Offset of the field `address`.
    #[inline]
    pub fn address_offset() -> i32 {
        offset_of!(Self, address) as i32
    }

    /// Offset of the field `sig_idx`.
    #[inline]
    pub fn sig_idx_offset() -> i32 {
        offset_of!(Self, sig_idx) as i32
    }
}

impl VmTable {
    /// Offset of the field `base_address`.
    #[inline]
    pub fn base_address_offset() -> i32 {
        offset_of!(Self, base_address) as i32
    }

    /// Offset of the field `amount_items`.
    #[inline]
    pub fn amount_items_offset() -> i32 {
        offset_of!(Self, amount_items) as i32
    }
}

impl VmTableElement {
    /// Null.
    pub fn null() -> Self {
        Self {
            address: VirtAddr::null(),
            sig_idx: core::u64::MAX, // Important: check func_env
        }
    }

    /// Creates a new table element.
    pub fn new(address: VirtAddr, sig_idx: SignatureIndex) -> Self {
        Self {
            address,
            sig_idx: sig_idx.as_u32() as u64,
        }
    }
}

impl VmContext {
    /// Heap offset in the context.
    pub fn heap_offset() -> i32 {
        offset_of!(VmContext, heap_ptr) as i32
    }

    /// Heap offset field size.
    pub const fn heap_offset_field_size() -> i32 {
        size_of::<VirtAddr>() as i32
    }

    /// Offset of the globals.
    pub fn globals_offset() -> i32 {
        Self::heap_offset() + Self::heap_offset_field_size()
    }

    /// Offset of a global entry.
    pub fn global_entry_offset(index: u32) -> isize {
        Self::globals_offset() as isize + (size_of::<VmGlobal>() * index as usize) as isize
    }

    /// Offset of imported functions.
    pub fn imported_funcs_offset(num_globals: u32) -> isize {
        Self::global_entry_offset(num_globals)
    }

    /// Offset of an imported function entry.
    pub fn imported_func_entry_offset(num_globals: u32, index: u32) -> isize {
        Self::imported_funcs_offset(num_globals) as isize
            + (size_of::<VmFunctionImportEntry>() * index as usize) as isize
    }

    /// Offset of the tables.
    pub fn tables_offset(num_globals: u32, num_imported_funcs: u32) -> isize {
        Self::imported_func_entry_offset(num_globals, num_imported_funcs)
    }

    /// Offset of a table.
    pub fn table_entry_offset(num_globals: u32, num_imported_funcs: u32, index: u32) -> isize {
        Self::tables_offset(num_globals, num_imported_funcs)
            + (index as usize * size_of::<VmTable>()) as isize
    }

    /// Calculates the size of the context.
    pub fn size(num_globals: u32, num_imported_funcs: u32, num_tables: u32) -> usize {
        Self::table_entry_offset(num_globals, num_imported_funcs, num_tables) as usize
    }
}

#[allow(clippy::cast_ptr_alignment)]
impl VmContextContainer {
    /// Creates a new container for a VmContext.
    pub unsafe fn new(
        heap: VirtAddr,
        num_globals: u32,
        num_imported_funcs: u32,
        tables: Vec<Table>,
    ) -> Self {
        // Allocate the memory for the VmContext.
        let layout = Self::layout(num_globals, num_imported_funcs, tables.len() as u32);
        let ptr = alloc(layout);
        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        // Set the heap pointer here already.
        let heap_ptr = ptr.offset(VmContext::heap_offset() as isize) as *mut VirtAddr;
        *heap_ptr = heap;

        Self {
            ptr: VirtAddr::from(ptr),
            num_imported_funcs,
            num_globals,
            tables,
        }
    }

    /// Gets the pointer to the context.
    pub fn ptr(&self) -> *const VmContext {
        self.ptr.as_const::<VmContext>()
    }

    /// Gets the raw u8 mutable pointer to the context.
    pub fn ptr_mut_u8(&mut self) -> *mut u8 {
        self.ptr.as_mut::<u8>()
    }

    /// Gets the function imports as a slice.
    /// Unsafe because you might be able to get multiple mutable references.
    pub unsafe fn function_imports_as_mut_slice(&mut self) -> &mut [VmFunctionImportEntry] {
        // Safety: we allocated the memory correctly and the bounds are correct at this point.
        let ptr = self
            .ptr_mut_u8()
            .offset(VmContext::imported_funcs_offset(self.num_globals) as isize)
            as *mut VmFunctionImportEntry;
        slice::from_raw_parts_mut(ptr, self.num_imported_funcs as usize)
    }

    /// Gets a mut slice to the tables.
    pub fn get_table(&mut self, idx: TableIndex) -> &mut Table {
        &mut self.tables[idx.as_u32() as usize]
    }

    /// Write the table data to the VmContext.
    pub fn write_tables_to_vmctx(&mut self) {
        // Safety: we allocated the memory correctly and the bounds are correct at this point.
        let vm_tables = unsafe {
            let ptr = self.ptr_mut_u8().offset(VmContext::tables_offset(
                self.num_globals,
                self.num_imported_funcs,
            )) as *mut VmTable;
            slice::from_raw_parts_mut(ptr, self.tables.len())
        };

        for (table, vm_table) in self.tables.iter().zip(vm_tables.iter_mut()) {
            *vm_table = table.as_vm_table();
        }
    }

    /// Sets a global.
    /// Unsafe because index might be outside bounds.
    pub unsafe fn set_global(&mut self, idx: u32, global: &Global) {
        debug_assert!(idx < self.num_globals);
        let ptr = self
            .ptr_mut_u8()
            .offset(VmContext::global_entry_offset(idx));

        match global.initializer {
            GlobalInit::I32Const(v) => (ptr as *mut i32).write(v),
            _ => unimplemented!(),
        }
    }

    /// Calculates the allocation layout of the VmContext.
    fn layout(num_globals: u32, num_imported_funcs: u32, num_tables: u32) -> Layout {
        let size = VmContext::size(num_globals, num_imported_funcs, num_tables);
        let align = align_of::<VmContext>();
        Layout::from_size_align(size, align).unwrap()
    }
}

impl Drop for VmContextContainer {
    fn drop(&mut self) {
        unsafe {
            dealloc(
                self.ptr_mut_u8(),
                Self::layout(
                    self.num_globals,
                    self.num_imported_funcs,
                    self.tables.len() as u32,
                ),
            );
        }
    }
}
