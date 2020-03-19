use crate::arch::address::VirtAddr;
use crate::arch::paging::PAGE_SIZE;
use alloc::alloc::{alloc, dealloc, handle_alloc_error};
use core::alloc::Layout;
use core::mem::{align_of, size_of};
use core::slice;

pub const HEAP_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4 GiB

pub const HEAP_GUARD_SIZE: u64 = PAGE_SIZE as u64;

#[repr(C)]
pub struct VmFunctionImportEntry {
    pub address: VirtAddr,
}

#[repr(C)]
pub struct VmTable {
    pub base_address: VirtAddr,
}

pub struct VmContext {}

pub struct VmContextContainer {
    ptr: *mut VmContext,
    num_imported_funcs: u32,
    num_tables: u32,
}

impl VmContext {
    /// Heap offset in the context.
    pub const fn heap_offset() -> i32 {
        0
    }

    /// Heap offset field size.
    pub const fn heap_offset_field_size() -> i32 {
        size_of::<usize>() as i32
    }

    /// Offset of imported functions.
    pub const fn imported_funcs_offset() -> i32 {
        Self::heap_offset() + Self::heap_offset_field_size()
    }

    /// Offset of an imported function entry.
    pub const fn imported_func_entry_offset(index: u32) -> isize {
        Self::imported_funcs_offset() as isize
            + (size_of::<VmFunctionImportEntry>() * index as usize) as isize
    }

    /// Offset of the tables.
    pub const fn table_offset(num_imported_funcs: u32) -> isize {
        Self::imported_func_entry_offset(num_imported_funcs)
    }

    /// Calculates the size of the context.
    pub const fn size(num_imported_funcs: u32, num_tables: u32) -> usize {
        Self::imported_funcs_offset() as usize
            + (num_imported_funcs as usize) * size_of::<VmFunctionImportEntry>()
            + (num_tables as usize) * size_of::<VmTable>()
    }
}

#[allow(clippy::cast_ptr_alignment)]
impl VmContextContainer {
    /// Creates a new container for a VmContext.
    pub unsafe fn new(heap: VirtAddr, num_imported_funcs: u32, num_tables: u32) -> Self {
        let layout = Self::layout(num_imported_funcs, num_tables);
        let ptr = alloc(layout);
        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        let heap_ptr = ptr.offset(VmContext::heap_offset() as isize) as *mut VirtAddr;
        *heap_ptr = heap;

        Self {
            ptr: ptr as *mut _,
            num_imported_funcs,
            num_tables,
        }
    }

    /// Gets the pointer to the context.
    pub fn ptr(&self) -> *const VmContext {
        self.ptr
    }

    /// Gets the function imports as a slice.
    /// Unsafe because you might be able to get multiple mutable references.
    pub unsafe fn function_imports_as_mut_slice(&mut self) -> &mut [VmFunctionImportEntry] {
        // Safety: we allocated the memory correctly and the bounds are correct at this point.
        let ptr = (self.ptr as *mut u8).offset(VmContext::imported_funcs_offset() as isize)
            as *mut VmFunctionImportEntry;
        slice::from_raw_parts_mut(ptr, self.num_imported_funcs as usize)
    }

    /// Gets the tables as a slice.
    /// Unsafe because you might be able to get multiple mutable references.
    pub unsafe fn tables_as_mut_slice(&mut self) -> &mut [VmTable] {
        // Safety: we allocated the memory correctly and the bounds are correct at this point.
        let ptr = (self.ptr as *mut u8).offset(VmContext::table_offset(self.num_imported_funcs))
            as *mut VmTable;
        slice::from_raw_parts_mut(ptr, self.num_tables as usize)
    }

    /// Calculates the allocation layout of the context.
    fn layout(num_imported_funcs: u32, num_tables: u32) -> Layout {
        let size = VmContext::size(num_imported_funcs, num_tables);
        let align = align_of::<Self>();
        Layout::from_size_align(size, align).unwrap()
    }
}

impl Drop for VmContextContainer {
    fn drop(&mut self) {
        unsafe {
            dealloc(
                self.ptr.cast(),
                Self::layout(self.num_imported_funcs, self.num_tables),
            );
        }
    }
}
