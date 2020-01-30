use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{null_mut, NonNull};
use core::mem::size_of;
use spin::Mutex;
use crate::arch::address::VirtAddr;
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::arch::paging::PAGE_SIZE;
use crate::mm::buddy::Tree;
use crate::mm::mapper::MemoryMapper;
use crate::util::unchecked::UncheckedUnwrap;

// TODO: free "free slabs" once there are a couple
// TODO: more efficient realloc, now uses the default, which always copies...
// TODO: less high orders? (acceptable fragmentation threshold?)

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

/// A slab for a cache.
#[derive(Debug)]
struct Slab {
    /// Maintain a linked list of slabs.
    next: Option<NonNull<Slab>>,
    prev: Option<NonNull<Slab>>,
    /// Next free offset, 0 if no more free space.
    next_offset: u32,
    /// Amount of free items.
    free_count: u32,
}

/// A cache in the slab allocator.
#[derive(Debug)]
struct Cache {
    partial: Option<NonNull<Slab>>,
    free: Option<NonNull<Slab>>,
    obj_size: u32,
    slots_count: u32,
    slab_order: u8,
    // TODO: color stuff
}

impl Slab {
    /// Inits the slab.
    pub fn init(&mut self, start_offset: u32, slots_count: u32, obj_size: u32) {
        self.next = None;
        self.prev = None;
        self.next_offset = start_offset;
        self.free_count = slots_count;

        unsafe {
            // Initialize free list.
            for i in 0..(slots_count - 1) {
                *self.ptr_at(start_offset + i * obj_size) = start_offset + (i + 1) * obj_size;
            }

            // Last entry is set to 0 to indicate end of free list.
            *self.ptr_at(start_offset + (slots_count - 1) * obj_size) = 0;
        }
    }

    /// Pointer to self
    #[inline]
    fn self_ptr(&mut self) -> *mut u8 {
        self as *mut Slab as *mut u8
    }

    /// Gets a pointer at an offset from this struct.
    fn ptr_at(&mut self, offset: u32) -> *mut u32 {
        debug_assert!(offset % 4 == 0);
        #[allow(clippy::cast_ptr_alignment)]
            unsafe { self.self_ptr().offset(offset as isize) as *mut u32 }
    }

    /// Unlink from next slab.
    pub fn unlink_from_next(&mut self) -> Option<NonNull<Slab>> {
        if self.next.is_some() {
            unsafe { self.next.unwrap().as_mut() }.prev = None;
            self.next.take()
        } else {
            None
        }
    }

    /// Allocate inside the slab.
    pub fn alloc(&mut self) -> *mut u8 {
        debug_assert!(!self.is_full());

        let allocated_offset = self.next_offset;
        self.next_offset = unsafe { *self.ptr_at(self.next_offset) };
        self.free_count -= 1;

        self.ptr_at(allocated_offset) as *mut u8
    }

    /// Deallocate inside the slab.
    pub fn dealloc(&mut self, ptr: *mut u8) {
        let allocated_offset = ptr as usize - self.self_ptr() as usize;
        debug_assert!(ptr as usize % 4 == 0);
        #[allow(clippy::cast_ptr_alignment)]
            let allocated_offset_ptr = ptr as *mut u32;
        unsafe { *allocated_offset_ptr = self.next_offset; }
        self.next_offset = allocated_offset as u32;
        self.free_count += 1;
    }

    /// Is full?
    pub fn is_full(&self) -> bool {
        debug_assert!((self.next_offset == 0) == (self.free_count == 0));
        self.next_offset == 0
    }
}

impl Cache {
    /// Creates a new cache.
    const fn new(obj_size: u32, slots_count: u32, slab_order: u8) -> Self {
        Self {
            partial: None,
            free: None,
            obj_size,
            slots_count,
            slab_order,
        }
    }

    /// Create a new slab and allocate from there.
    fn alloc_new_slab(&mut self, heap: &mut Heap) -> *mut u8 {
        // Create a new slab to allocate from. This will become a partial slab.
        if let Some(slab) = heap.create_free_slab(self.slab_order as usize, 24 /* TODO */, self.slots_count, self.obj_size) {
            let result = slab.alloc();

            // There were no partial or free slabs, otherwise we would've allocated from there.
            self.partial = NonNull::new(slab);

            result
        } else {
            null_mut()
        }
    }

    /// Allocate.
    fn alloc(&mut self, heap: &mut Heap) -> *mut u8 {
        /*
         * Try to allocate from partial slabs first.
         * If there are none, try the free slabs.
         * If there are no free slabs, we have to create a new slab.
         */
        if let Some(mut tmp) = self.partial {
            // Reference is valid and not shared.
            let slab = unsafe { tmp.as_mut() };
            debug_assert!(slab.prev.is_none());

            // Cannot fail, because otherwise it wouldn't be a partial slab!
            let result = slab.alloc();

            // Do we still have slots left? If not, this became a full slab instead of a partial.
            if slab.is_full() {
                // Remove from linked list.
                self.partial = slab.unlink_from_next();
            }

            result
        } else if let Some(mut tmp) = self.free {
            // Reference is valid and not shared.
            let slab = unsafe { tmp.as_mut() };
            debug_assert!(slab.prev.is_none());

            // Cannot fail, because otherwise it wouldn't be a free slab!
            let result = slab.alloc();

            // Since this now holds an object, this became a partial slab.
            // We also know there are no partial slabs atm, because we always try partials first.
            self.free = slab.unlink_from_next();
            self.partial = NonNull::new(slab);

            result
        } else {
            self.alloc_new_slab(heap)
        }
    }

    /// Deallocate.
    fn dealloc(&mut self, heap: &Heap, ptr: *mut u8) {
        // First, figure out which slab it was from.
        // The slab is aligned at a multiple of 2^order pages.
        let offset = ptr as usize - heap.alloc_area_start.as_usize();
        let alignment = PAGE_SIZE << self.slab_order as usize;
        let slab_addr = heap.alloc_area_start.as_usize() + (offset & !(alignment - 1));
        let slab = unsafe { &mut *(slab_addr as *mut Slab) };
        let old_free_count = slab.free_count;

        // Can now deallocate
        slab.dealloc(ptr);

        // Update partial & free pointers
        if old_free_count == 0 {
            // It became a partial, and it was full, so it wasn't linked to.
            debug_assert!(slab.next.is_none());
            debug_assert!(slab.prev.is_none());
            slab.next = self.partial;
            self.partial = NonNull::new(slab);
        } else {
            // It was linked, either as a free or as a partial slab.
            if old_free_count + 1 == self.slots_count {
                //println!("Convert partial to free slab");

                // It was a partial slab and it became a free slab.
                if let Some(mut next) = slab.next {
                    unsafe { next.as_mut() }.prev = slab.prev;
                }
                if let Some(mut prev) = slab.prev {
                    unsafe { prev.as_mut() }.next = slab.next;
                } else {
                    // No previous, so must be the first.
                    self.partial = slab.next;
                }
                slab.next = self.free;
                self.free = NonNull::new(slab);
            }
        }
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

    /// Create cache.
    fn create_cache(obj_size: usize, alignment: usize) -> Cache {
        fn align(x: usize, alignment: usize) -> usize {
            ((x + alignment - 1) / alignment) * alignment
        }

        let obj_size = align(obj_size, alignment);
        let slab_rounded_up = align(size_of::<Slab>(), alignment);

        const MAX: usize = 5;
        let mut best_wastage = PAGE_SIZE;
        let mut order = 0;
        let mut slots_count = 0;

        for i in 0..=MAX {
            let size = (PAGE_SIZE << i) - slab_rounded_up;
            let wastage = size % obj_size;

            if wastage < best_wastage {
                slots_count = size / obj_size;
                best_wastage = wastage;
                order = i;
            }
        }

        Cache::new(obj_size as u32, slots_count as u32, order as u8)
    }

    /// Creates a free slab of the requested order.
    pub fn create_free_slab<'s>(&mut self, order: usize, start_offset: u32, slots_count: u32, obj_size: u32)
                                -> Option<&'s mut Slab> {
        let offset = self.tree.alloc(order)?;
        let addr = self.alloc_area_start + offset * PAGE_SIZE;
        let size = PAGE_SIZE << order;
        let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;

        if unlikely!(ActiveMapping::get().map_range(addr, size, flags).is_err()) {
            self.tree.dealloc(offset);
            None
        } else {
            let slab = unsafe { &mut *(addr.as_usize() as *mut Slab) };
            slab.init(start_offset, slots_count, obj_size);
            Some(slab)
        }
    }

    /// Allocate.
    pub fn alloc(&self, layout: Layout) -> *mut u8 {
        null_mut()
    }

    /// Deallocate.
    pub fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unimplemented!()
    }

    pub fn test(&mut self) { // TODO
        let mut cache = Heap::create_cache(80, 16);
        println!("{:?}", cache);

        for _ in 0..49 {
            cache.alloc(self);
        }

        let a = cache.alloc(self);
        println!("{:?}", a);
        let b = cache.alloc(self);
        println!("{:?}", b);
        let c = cache.alloc(self);
        println!("{:?}", c);
        let d = cache.alloc(self);
        println!("{:?}", d);
        println!("{:?}", cache);
        cache.dealloc(self, c);
        cache.dealloc(self, b);
        let d = cache.alloc(self);
        println!("{:?}", d);
        let d = cache.alloc(self);
        println!("{:?}", d);
        let d = cache.alloc(self);
        println!("{:?}", d);

        loop {}
    }
}

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
