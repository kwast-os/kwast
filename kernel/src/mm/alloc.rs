use core::alloc::GlobalAlloc;
use bitflags::_core::alloc::Layout;
use bitflags::_core::ptr::null_mut;

struct Dummy;

// TODO: more efficient realloc
unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unimplemented!()
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

#[global_allocator]
static ALLOCATOR: Dummy = Dummy {};
