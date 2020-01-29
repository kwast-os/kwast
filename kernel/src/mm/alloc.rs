use core::alloc::GlobalAlloc;
use bitflags::_core::alloc::Layout;
use bitflags::_core::ptr::null_mut;
use crate::mm::buddy::Tree;
use spin::Mutex;
use crate::arch::address::VirtAddr;
use crate::util::unchecked::UncheckedUnwrap;
use crate::arch::x86_64::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use core::mem::size_of;

struct Heap {
    /// Tree that can be used to get a contiguous area of pages for the slabs.
    /// Currently there is only one tree, but this can be extended in the future to use multiple.
    tree: &'static mut Tree,
}

struct LockedHeap {
    /// Inner heap.
    inner: Mutex<Option<Heap>>,
}

struct Slab {
    /// Maintain a linked list of slabs.
    next: Option<&'static Slab>,
}

/// A cache in the slab allocator.
struct Cache {
    partial: Option<&'static Slab>,
    free: Option<&'static Slab>,
    // TODO: color stuff
}

impl Slab {}

impl Cache {
    /// Creates a new cache.
    fn new() -> Self {
        Self {
            partial: None,
            free: None,
        }
    }

    fn create_free_slab() {
        // TODO
    }

    // TODO: alloc_from_slab

    fn alloc(&self) {
        // TODO

        if self.partial.is_none() {}
    }
}

impl Heap {
    /// Creates a new heap.
    pub fn new(tree_location: VirtAddr) -> Self {
        // Map space for the tree
        println!("Tree at {:?}", tree_location);
        let mut mapping = ActiveMapping::get();
        mapping.map_range(tree_location, size_of::<Tree>(), EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX);

        // Create the tree
        let tree = unsafe { &mut *(tree_location.as_usize() as *mut Tree) };
        tree.init();

        Heap {
            tree,
        }
    }
}

// TODO: more efficient realloc
unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unimplemented!()
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    // TODO: handle this better
    panic!("allocation error: {:?}", layout)
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap { inner: Mutex::new(None) };

/// Inits allocation.
pub fn init(reserved_end: VirtAddr) {
    *ALLOCATOR.inner.lock() = Some(Heap::new(reserved_end));

    let c = Cache::new();

    c.alloc();
}
