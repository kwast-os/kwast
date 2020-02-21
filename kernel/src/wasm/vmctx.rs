use crate::arch::address::VirtAddr;
use crate::arch::paging::PAGE_SIZE;
use alloc::alloc::{alloc, dealloc, handle_alloc_error};
use core::alloc::Layout;
use core::mem::{align_of, size_of};

pub const HEAP_SIZE: u64 = 4 * 1024 * 1024 * 1024; // 4 GiB

pub const HEAP_GUARD_SIZE: u64 = PAGE_SIZE as u64;

pub struct VmContext {}

#[derive(Debug)]
pub struct VmContextContainer {
    ptr: *mut VmContext,
    num_imported_funcs: u32,
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
    pub const fn imported_func_entry_offset(index: u32) -> usize {
        Self::imported_funcs_offset() as usize + size_of::<usize>() * index as usize
    }

    /// Calculates the size of the context.
    pub const fn size(num_imported_funcs: u32) -> usize {
        Self::imported_funcs_offset() as usize + (num_imported_funcs as usize) * size_of::<usize>()
    }
}

#[allow(clippy::cast_ptr_alignment)]
impl VmContextContainer {
    /// Creates a new container for a VmContext.
    pub unsafe fn new(heap: VirtAddr, num_imported_funcs: u32) -> VmContextContainer {
        let layout = Self::layout(num_imported_funcs);
        let ptr = alloc(layout);
        if ptr.is_null() {
            handle_alloc_error(layout);
        }

        let heap_ptr = ptr.offset(VmContext::heap_offset() as isize) as *mut VirtAddr;
        *heap_ptr = heap;

        Self {
            ptr: ptr as *mut _,
            num_imported_funcs,
        }
    }

    /// Gets the pointer to the context.
    pub fn ptr(&self) -> *const VmContext {
        self.ptr
    }

    pub fn set_function_import(&mut self, index: u32, address: VirtAddr) {
        debug_assert!(index < self.num_imported_funcs);
        unsafe {
            let ptr = (self.ptr as *mut u8).offset(VmContext::imported_funcs_offset() as isize)
                as *mut VirtAddr;
            *ptr = address;
        }
    }

    /// Calculates the allocation layout of the context.
    fn layout(num_imported_funcs: u32) -> Layout {
        let size = VmContext::size(num_imported_funcs);
        let align = align_of::<Self>();
        Layout::from_size_align(size, align).unwrap()
    }
}

impl Drop for VmContextContainer {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.cast(), Self::layout(self.num_imported_funcs));
        }
    }
}
