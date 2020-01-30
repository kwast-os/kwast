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

struct HeapManager {
    /// Tree that can be used to get a contiguous area of pages for the slabs.
    /// Currently there is only one tree, but this can be extended in the future to use multiple.
    tree: &'static mut Tree,
    /// Allocation area start.
    alloc_area_start: VirtAddr,
}

struct HeapCaches {
    /// Generic size caches.
    cache32: Cache,
    cache64: Cache,
    cache128: Cache,
    cache256: Cache,
    cache512: Cache,
    cache1024: Cache,
    cache2048: Cache,
    cache4096: Cache,
    cache8192: Cache,
}

/// The heap.
struct Heap {
    /// Heap management for expansion and shrinking.
    manager: HeapManager,
    /// Caches.
    caches: HeapCaches,
}

/// Newtype for slab linking.
#[derive(Debug)]
struct SlabLink(Option<NonNull<Slab>>);

impl SlabLink {
    fn is_none(self) -> bool {
        self.0.is_none()
    }

    fn is_some(self) -> bool {
        self.0.is_some()
    }

    fn take(&mut self) -> SlabLink {
        SlabLink(self.0.take())
    }

    fn unwrap(self) -> NonNull<Slab> {
        self.0.unwrap()
    }
}

impl Clone for SlabLink {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for SlabLink {}

/// Private, and will always be behind a lock. Does not contain thread-local information.
unsafe impl Send for SlabLink {}

/// A wrapper around the heap to lock the inner heap.
struct LockedHeap {
    /// Inner heap.
    inner: Mutex<Option<Heap>>,
}

/// A slab for a cache.
#[derive(Debug)]
struct Slab {
    /// Maintain a linked list of slabs.
    next: SlabLink,
    prev: SlabLink,
    /// Next free offset, 0 if no more free space.
    next_offset: u32,
    /// Amount of free items.
    free_count: u32,
}

/// A cache in the slab allocator.
#[derive(Debug)]
struct Cache {
    partial: SlabLink,
    free: SlabLink,
    obj_size: u32,
    slots_count: u32,
    start_offset: u32,
    alignment: u32,
    color: u16,
    max_color: u16,
    slab_order: u8,
}

impl Slab {
    /// Inits the slab.
    pub fn init(&mut self, start_offset: u32, slots_count: u32, obj_size: u32) {
        debug_assert!(slots_count > 1);
        self.next = SlabLink(None);
        self.prev = SlabLink(None);
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
    #[allow(clippy::cast_ptr_alignment)]
    fn ptr_at(&mut self, offset: u32) -> *mut u32 {
        debug_assert!(offset % 4 == 0);
        unsafe { self.self_ptr().offset(offset as isize) as *mut u32 }
    }

    /// Unlink from next slab.
    pub fn unlink_from_next(&mut self) -> SlabLink {
        if self.next.is_some() {
            unsafe { self.next.unwrap().as_mut() }.prev = SlabLink(None);
            self.next.take()
        } else {
            SlabLink(None)
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
    #[allow(clippy::cast_ptr_alignment)]
    pub fn dealloc(&mut self, ptr: *mut u8) {
        let allocated_offset = ptr as usize - self.self_ptr() as usize;
        debug_assert!(ptr as usize % 4 == 0);
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
    fn new(obj_size: u32, alignment: u32, max_color: u16, start_offset: u32, slots_count: u32, slab_order: u8) -> Self {
        Self {
            partial: SlabLink(None),
            free: SlabLink(None),
            obj_size,
            slots_count,
            start_offset,
            alignment,
            color: 0,
            max_color,
            slab_order,
        }
    }

    /// Create cache.
    fn calculate_and_create(obj_size: usize, alignment: usize) -> Cache {
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
            if PAGE_SIZE << i < obj_size {
                continue;
            }

            let size = (PAGE_SIZE << i) - slab_rounded_up;
            let wastage = size % obj_size;

            if wastage < best_wastage {
                slots_count = size / obj_size;
                best_wastage = wastage;
                order = i;
            }
        }

        let max_color = (best_wastage / alignment) * alignment;
        //println!("best_wastage: {}, max_color: {}", best_wastage, max_color);

        Cache::new(obj_size as u32, alignment as u32, max_color as u16, slab_rounded_up as u32, slots_count as u32, order as u8)
    }

    /// Create a new slab and allocate from there.
    fn alloc_new_slab(&mut self, heap: &mut HeapManager) -> *mut u8 {
        let start_offset = self.start_offset + self.color as u32;
        self.color += self.alignment as u16;
        if self.color > self.max_color {
            self.color = 0;
        }

        // Create a new slab to allocate from. This will become a partial slab.
        if let Some(slab) = heap.create_free_slab(self.slab_order as usize, start_offset, self.slots_count, self.obj_size) {
            let result = slab.alloc();

            // There were no partial or free slabs, otherwise we would've allocated from there.
            self.partial = SlabLink(NonNull::new(slab));

            result
        } else {
            null_mut()
        }
    }

    /// Allocate.
    fn alloc(&mut self, heap: &mut HeapManager) -> *mut u8 {
        /*
         * Try to allocate from partial slabs first.
         * If there are none, try the free slabs.
         * If there are no free slabs, we have to create a new slab.
         */
        if let SlabLink(Some(mut tmp)) = self.partial {
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
        } else if let SlabLink(Some(mut tmp)) = self.free {
            // Reference is valid and not shared.
            let slab = unsafe { tmp.as_mut() };
            debug_assert!(slab.prev.is_none());

            // Cannot fail, because otherwise it wouldn't be a free slab!
            let result = slab.alloc();

            // Since this now holds an object, this became a partial slab.
            // We also know there are no partial slabs atm, because we always try partials first.
            self.free = slab.unlink_from_next();
            self.partial = SlabLink(NonNull::new(slab));

            result
        } else {
            self.alloc_new_slab(heap)
        }
    }

    /// Deallocate.
    fn dealloc(&mut self, heap: &HeapManager, ptr: *mut u8) {
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
            self.partial = SlabLink(NonNull::new(slab));
        } else {
            // It was linked, either as a free or as a partial slab.
            if old_free_count + 1 == self.slots_count {
                //println!("Convert partial to free slab");

                // It was a partial slab and it became a free slab.
                if let SlabLink(Some(mut next)) = slab.next {
                    unsafe { next.as_mut() }.prev = slab.prev;
                }
                if let SlabLink(Some(mut prev)) = slab.prev {
                    unsafe { prev.as_mut() }.next = slab.next;
                } else {
                    // No previous, so must be the first.
                    self.partial = slab.next;
                }
                slab.next = self.free;
                self.free = SlabLink(NonNull::new(slab));
            }
        }
    }
}

impl HeapManager {
    /// Creates a new manager.
    pub fn new(tree_location: VirtAddr) -> Self {
        // Map space for the tree
        let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
        let mut mapping = ActiveMapping::get();
        mapping.map_range(tree_location, size_of::<Tree>(), flags).unwrap();

        // Create the tree
        let tree = unsafe { &mut *(tree_location.as_usize() as *mut Tree) };
        tree.init();

        Self {
            tree,
            alloc_area_start: (tree_location + size_of::<Tree>()).align_up(),
        }
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
}

impl HeapCaches {
    /// Converts a layout to a cache.
    fn layout_to_cache(&mut self, layout: Layout) -> &mut Cache {
        if layout.size() <= 32 && layout.align() <= 32 {
            &mut self.cache32
        } else if layout.size() <= 64 && layout.align() <= 64 {
            &mut self.cache64
        } else if layout.size() <= 128 && layout.align() <= 128 {
            &mut self.cache128
        } else if layout.size() <= 256 && layout.align() <= 256 {
            &mut self.cache256
        } else if layout.size() <= 512 && layout.align() <= 512 {
            &mut self.cache512
        } else if layout.size() <= 1024 && layout.align() <= 1024 {
            &mut self.cache1024
        } else if layout.size() <= 2048 && layout.align() <= 2048 {
            &mut self.cache2048
        } else if layout.size() <= 4096 && layout.align() <= 4096 {
            &mut self.cache4096
        } else {
            debug_assert!(layout.size() <= 8192 && layout.align() <= 8192);
            &mut self.cache8192
        }
    }
}

impl Heap {
    /// Creates a new heap.
    pub fn new(tree_location: VirtAddr) -> Self {
        Heap {
            manager: HeapManager::new(tree_location),
            caches: HeapCaches {
                cache32: Cache::calculate_and_create(32, 32),
                cache64: Cache::calculate_and_create(64, 64),
                cache128: Cache::calculate_and_create(128, 128),
                cache256: Cache::calculate_and_create(256, 256),
                cache512: Cache::calculate_and_create(512, 512),
                cache1024: Cache::calculate_and_create(1024, 1024),
                cache2048: Cache::calculate_and_create(2048, 2048),
                cache4096: Cache::calculate_and_create(4096, 4096),
                cache8192: Cache::calculate_and_create(8192, 8192),
            },
        }
    }

    /// Allocate.
    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        // TODO: big allocations

        self.caches.layout_to_cache(layout).alloc(&mut self.manager)
    }

    /// Deallocate.
    pub fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        // TODO: big deallocations

        self.caches.layout_to_cache(layout).dealloc(&self.manager, ptr)
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.lock().as_mut().unchecked_unwrap().alloc(layout)
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().as_mut().unchecked_unwrap().dealloc(ptr, layout)
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
    debug_assert!(ALLOCATOR.inner.lock().is_none());
    *ALLOCATOR.inner.lock() = Some(Heap::new(reserved_end));
}
