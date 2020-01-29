use core::alloc::GlobalAlloc;
use bitflags::_core::alloc::Layout;
use bitflags::_core::ptr::null_mut;
use crate::mm::buddy::Tree;
use spin::Mutex;
use crate::arch::address::VirtAddr;
use crate::util::unchecked::UncheckedUnwrap;
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use core::mem::size_of;
use crate::arch::x86_64::paging::PAGE_SIZE;

struct Heap {
    /// Tree that can be used to get a contiguous area of pages for the slabs.
    /// Currently there is only one tree, but this can be extended in the future to use multiple.
    tree: &'static mut Tree,
    /// Allocation area start
    alloc_area_start: VirtAddr,
}

struct LockedHeap {
    /// Inner heap.
    inner: Mutex<Option<Heap>>,
}

#[derive(Debug)]
struct Slab {
    /// Maintain a linked list of slabs.
    next: Option<&'static Slab>,
}

/// A cache in the slab allocator.
struct Cache {
    partial: Option<&'static Slab>,
    free: Option<&'static Slab>,
    slab_order: u8,
    // TODO: color stuff
}

impl Slab {
    /// Inits the slab.
    pub fn init(&mut self) {
        self.next = None;
    }
}

impl Cache {
    /// Creates a new cache.
    fn new(slab_order: u8) -> Self {
        Self {
            partial: None,
            free: None,
            slab_order,
        }
    }

    // TODO: alloc_from_slab

    fn alloc(&self, heap: &mut Heap) {
        // TODO

        let slab = heap.create_free_slab(self.slab_order as usize);
        println!("{:?}", slab);

        if self.partial.is_none() {}
    }
}

impl Heap {
    /// Creates a new heap.
    pub fn new(tree_location: VirtAddr) -> Self {
        // Map space for the tree
        let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
        let mut mapping = ActiveMapping::get();
        mapping.map_range(tree_location, size_of::<Tree>(), flags).unwrap();

        // Create the tree
        let tree = unsafe { &mut *(tree_location.as_usize() as *mut Tree) };
        tree.init();

        Heap {
            tree,
            alloc_area_start: (tree_location + size_of::<Tree>()).align_up(),
        }
    }

    /// Creates a free slab of the requested order.
    pub fn create_free_slab(&mut self, order: usize) -> Option<&'static Slab> {
        let offset = self.tree.alloc(order)?;
        let addr = self.alloc_area_start + offset * PAGE_SIZE;
        let size = (1 << order) * PAGE_SIZE;
        let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;

        if unlikely!(ActiveMapping::get().map_range(addr, size, flags).is_err()) {
            self.tree.dealloc(offset);
            None
        } else {
            let slab = unsafe { &mut *(addr.as_usize() as *mut Slab) };
            slab.init();
            Some(slab)
        }
    }

    pub fn alloc(&self, layout: Layout) -> *mut u8 {
        null_mut()
    }

    pub fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unimplemented!()
    }

    pub fn test(&mut self) { // TODO
        let cache = Cache::new(2);
        cache.alloc(self);
    }
}

// TODO: more efficient realloc, now uses the default, which always copies...
unsafe impl GlobalAlloc for LockedHeap {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.lock().as_ref().unchecked_unwrap().alloc(layout)
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().as_ref().unchecked_unwrap().dealloc(ptr, layout)
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

    ALLOCATOR.inner.lock().as_mut().unwrap().test();
}
