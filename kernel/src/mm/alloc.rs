use crate::arch;
use crate::arch::address::VirtAddr;
use crate::arch::paging::{ActiveMapping, EntryFlags, PAGE_SIZE};
use crate::mm::buddy;
use crate::mm::buddy::Tree;
use crate::mm::mapper::MemoryMapper;
use crate::sync::spinlock::Spinlock;
use crate::util::unchecked::UncheckedUnwrap;
use core::alloc::{GlobalAlloc, Layout};
use core::cmp;
use core::intrinsics::unlikely;
use core::mem::size_of;
use core::ptr::{null_mut, NonNull};

struct SpaceManager<'t> {
    /// Tree that can be used to get a contiguous area of pages for the slabs.
    /// Currently there is only one tree, but this can be extended in the future to use multiple.
    tree: &'t mut Tree,
    /// Allocation area start.
    alloc_area_start: VirtAddr,
}

#[derive(Debug)]
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
    /// Management for expansion and shrinking.
    space_manager: SpaceManager<'static>,
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
    inner: Spinlock<Option<Heap>>,
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
    free_slab_count: u8,
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
    fn unlink_from_next(&mut self) -> SlabLink {
        if self.next.is_some() {
            unsafe { self.next.unwrap().as_mut() }.prev = SlabLink(None);
            self.next.take()
        } else {
            SlabLink(None)
        }
    }

    /// Allocate inside the slab.
    fn alloc(&mut self) -> *mut u8 {
        debug_assert!(!self.is_full());

        let allocated_offset = self.next_offset;
        self.next_offset = unsafe { *self.ptr_at(self.next_offset) };
        self.free_count -= 1;

        self.ptr_at(allocated_offset) as *mut u8
    }

    /// Deallocate inside the slab.
    #[allow(clippy::cast_ptr_alignment)]
    fn dealloc(&mut self, ptr: *mut u8) {
        let allocated_offset = ptr as usize - self.self_ptr() as usize;
        debug_assert!(ptr as usize % 4 == 0);
        let allocated_offset_ptr = ptr as *mut u32;
        unsafe {
            *allocated_offset_ptr = self.next_offset;
        }
        self.next_offset = allocated_offset as u32;
        self.free_count += 1;
    }

    /// Is full?
    #[inline]
    fn is_full(&self) -> bool {
        debug_assert!((self.next_offset == 0) == (self.free_count == 0));
        self.next_offset == 0
    }
}

impl Cache {
    /// Creates a new cache.
    fn new(
        obj_size: u32,
        alignment: u32,
        max_color: u16,
        start_offset: u32,
        slots_count: u32,
        slab_order: u8,
    ) -> Self {
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
            free_slab_count: 0,
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
            if (PAGE_SIZE << i) < obj_size + slab_rounded_up {
                continue;
            }

            let size = (PAGE_SIZE << i) - slab_rounded_up;
            let wastage = size % obj_size;

            if wastage < best_wastage {
                slots_count = size / obj_size;
                if slots_count > 1 {
                    best_wastage = wastage;
                    order = i;
                }
            }
        }

        debug_assert!(slots_count > 1);

        let max_color = (best_wastage / alignment) * alignment;
        // println!(
        //     "best_wastage: {}, max_color: {}, order: {}, slots_count: {}",
        //     best_wastage, max_color, order, slots_count
        // );

        Cache::new(
            obj_size as u32,
            alignment as u32,
            max_color as u16,
            slab_rounded_up as u32,
            slots_count as u32,
            order as u8,
        )
    }

    /// Cleans up free slab(s).
    fn cleanup_free_slab(&mut self, space_manager: &mut SpaceManager) {
        debug_assert!(self.free_slab_count > 1);

        // Find oldest in chain.
        let mut oldest = unsafe {
            let mut it = self.free;
            while let SlabLink(Some(next)) = it.unwrap().as_ref().next {
                it = SlabLink(Some(next));
            }
            it.unwrap()
        };
        let oldest = unsafe { oldest.as_mut() };

        debug_assert!(oldest.next.is_none());

        // Cleanup oldest
        if let SlabLink(Some(mut prev)) = oldest.prev.take() {
            unsafe { prev.as_mut() }.next = SlabLink(None);
        } else {
            // No previous, so must be the first one in the chain.
            self.free = SlabLink(None);
        }
        self.free_slab_count -= 1;
        space_manager.free_slab(self.slab_order as usize, oldest);
    }

    /// Create a new slab and allocate from there.
    fn alloc_new_slab(&mut self, space_manager: &mut SpaceManager) -> *mut u8 {
        let start_offset = self.start_offset + self.color as u32;
        self.color += self.alignment as u16;
        if self.color > self.max_color {
            self.color = 0;
        }

        // Create a new slab to allocate from. This will become a partial slab.
        if let Some(slab) = space_manager.create_free_slab(
            self.slab_order as usize,
            start_offset,
            self.slots_count,
            self.obj_size,
        ) {
            let result = slab.alloc();

            // There were no partial or free slabs, otherwise we would've allocated from there.
            self.partial = SlabLink(NonNull::new(slab));

            result
        } else {
            null_mut()
        }
    }

    /// Allocate.
    fn alloc(&mut self, space_manager: &mut SpaceManager) -> *mut u8 {
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
            self.free_slab_count -= 1;
            self.partial = SlabLink(NonNull::new(slab));

            result
        } else {
            self.alloc_new_slab(space_manager)
        }
    }

    /// Deallocate.
    fn dealloc(&mut self, space_manager: &mut SpaceManager, ptr: *mut u8) {
        // First, figure out which slab it was from.
        // The slab is aligned at a multiple of 2^order pages.
        let offset = ptr as usize - space_manager.alloc_area_start.as_usize();
        let alignment = PAGE_SIZE << self.slab_order as usize;
        let slab_addr = space_manager.alloc_area_start.as_usize() + (offset & !(alignment - 1));
        let slab = unsafe { &mut *(slab_addr as *mut Slab) };

        // Can now deallocate
        slab.dealloc(ptr);

        // Update partial & free pointers
        if slab.free_count == 1 {
            // It became a partial, and it was full, so it wasn't linked to.
            debug_assert!(slab.next.is_none());
            debug_assert!(slab.prev.is_none());
            slab.next = self.partial;
            self.partial = SlabLink(NonNull::new(slab));

            if let SlabLink(Some(mut next)) = slab.next {
                unsafe { next.as_mut() }.prev = self.partial;
            }
        } else {
            // It was linked, either as a free or as a partial slab.
            // If it was a partial, it could have become a free slab.
            if slab.free_count == self.slots_count {
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
                slab.prev = SlabLink(None);
                self.free = SlabLink(NonNull::new(slab));

                if let SlabLink(Some(mut next)) = slab.next {
                    unsafe { next.as_mut() }.prev = self.free;
                }

                self.free_slab_count += 1;

                if self.free_slab_count >= 2 {
                    self.cleanup_free_slab(space_manager);
                }
            }
        }
    }
}

impl<'t> SpaceManager<'t> {
    /// Creates a new manager.
    fn new(tree_location: VirtAddr) -> Self {
        // Map space for the tree
        let flags =
            EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX | EntryFlags::GLOBAL;

        // Safety:
        // We are the only running thread right now, so no locking is required.
        // It's not possible to use the `thread.domain()` method anyway,
        // because scheduling and threading is not up yet since it depends on this.
        let mut mapping = unsafe { ActiveMapping::get_unlocked() };
        mapping
            .map_range(tree_location, size_of::<Tree>(), flags)
            .expect("cannot map range for tree");

        // Safety:
        // We own this and mapped this. The size is big enough.
        let tree = unsafe { Tree::from(tree_location) };

        Self {
            tree,
            alloc_area_start: (tree_location + size_of::<Tree>()).align_up(),
        }
    }

    /// This function should provide a safe way to get the active mapping. May lock.
    #[inline]
    fn get_active_mapping(&mut self) -> ActiveMapping {
        // Safety:
        // This is safe as long as we have a global heap lock,
        // because we have our very own area in the page structures which cannot be affected
        // by user processes directly.
        unsafe { ActiveMapping::get_unlocked() }
    }

    /// Maximum end address of the heap.
    fn max_end(&self) -> VirtAddr {
        // We currently only maintain one tree. This could be extended in the future for more.
        self.offset_to_addr(buddy::MAX_OFFSET + 1)
    }

    /// Offset to address.
    fn offset_to_addr(&self, offset: usize) -> VirtAddr {
        self.alloc_area_start + offset * PAGE_SIZE
    }

    /// Converts an offset to a pointer and map the area.
    fn offset_to_ptr_and_map(&mut self, order: usize, offset: usize) -> *mut u8 {
        let addr = self.offset_to_addr(offset);
        let size = PAGE_SIZE << order;
        let flags =
            EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX | EntryFlags::GLOBAL;

        if unlikely(
            self.get_active_mapping()
                .map_range(addr, size, flags)
                .is_err(),
        ) {
            self.tree.dealloc(order, offset);
            null_mut()
        } else {
            addr.as_mut()
        }
    }

    /// Unmaps the area assigned to the pointer.
    fn ptr_unmap(&mut self, order: usize, ptr: *mut u8) {
        let offset = (ptr as usize - self.alloc_area_start.as_usize()) / PAGE_SIZE;
        self.tree.dealloc(order, offset);

        let size = PAGE_SIZE << order;
        self.get_active_mapping()
            .free_and_unmap_range(VirtAddr::new(ptr as usize), size);
    }

    /// Creates a free slab of the requested order.
    fn create_free_slab<'s>(
        &mut self,
        order: usize,
        start_offset: u32,
        slots_count: u32,
        obj_size: u32,
    ) -> Option<&'s mut Slab> {
        let offset = self.tree.alloc(order)?;
        let ptr = self.offset_to_ptr_and_map(order, offset);

        if unlikely(ptr.is_null()) {
            None
        } else {
            let slab = unsafe { &mut *(ptr as usize as *mut Slab) };
            slab.init(start_offset, slots_count, obj_size);
            Some(slab)
        }
    }

    /// Free the slab.
    fn free_slab(&mut self, order: usize, slab: &mut Slab) {
        let ptr = slab as *mut Slab as *mut u8;
        self.ptr_unmap(order, ptr);
    }

    /// Allocate big chunk.
    fn alloc_big(&mut self, order: usize) -> *mut u8 {
        self.tree.alloc(order).map_or(null_mut(), |offset| {
            self.offset_to_ptr_and_map(order, offset)
        })
    }

    /// Deallocate big chunk.
    fn dealloc_big(&mut self, order: usize, ptr: *mut u8) {
        self.ptr_unmap(order, ptr)
    }
}

impl HeapCaches {
    const MAX_CACHE: usize = 8192;

    /// Converts a layout to a cache.
    fn size_to_cache(&mut self, alloc_size: usize) -> &mut Cache {
        match alloc_size {
            32 => &mut self.cache32,
            64 => &mut self.cache64,
            128 => &mut self.cache128,
            256 => &mut self.cache256,
            512 => &mut self.cache512,
            1024 => &mut self.cache1024,
            2048 => &mut self.cache2048,
            4096 => &mut self.cache4096,
            8192 => &mut self.cache8192,
            _ => unreachable!(),
        }
    }

    /// Converts the layout to a type.
    #[allow(clippy::assertions_on_constants)]
    fn layout_to_size(layout: Layout) -> usize {
        // Note that alignment bigger than 4096 is not possible because the start of the slabs
        // are only aligned on at least a page, but not necessarily more than one.
        debug_assert!(PAGE_SIZE >= 4096);

        if layout.size() <= 32 && layout.align() <= 32 {
            32
        } else if layout.size() <= 64 && layout.align() <= 64 {
            64
        } else if layout.size() <= 128 && layout.align() <= 128 {
            128
        } else if layout.size() <= 256 && layout.align() <= 256 {
            256
        } else if layout.size() <= 512 && layout.align() <= 512 {
            512
        } else if layout.size() <= 1024 && layout.align() <= 1024 {
            1024
        } else if layout.size() <= 2048 && layout.align() <= 2048 {
            2048
        } else if layout.size() <= 4096 && layout.align() <= 4096 {
            4096
        } else if layout.size() <= 8192 && layout.align() <= 4096 {
            8192
        } else {
            layout.size()
        }
    }
}

impl Heap {
    /// Creates a new heap.
    fn new(tree_location: VirtAddr) -> Self {
        Heap {
            space_manager: SpaceManager::new(tree_location),
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

    /// Maximum end address of the heap.
    fn max_end(&self) -> VirtAddr {
        self.space_manager.max_end()
    }

    /// Converts a size to an order.
    fn size_to_order(size: usize) -> usize {
        let mut size = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        size -= 1;
        size |= size >> 1;
        size |= size >> 2;
        size |= size >> 4;
        size |= size >> 8;
        size |= size >> 16;
        size |= size >> 32;
        size += 1;

        63 - size.leading_zeros() as usize
    }

    /// Allocate.
    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let alloc_size = HeapCaches::layout_to_size(layout);
        let ptr = if alloc_size > HeapCaches::MAX_CACHE {
            self.space_manager
                .alloc_big(Self::size_to_order(layout.size()))
        } else {
            self.caches
                .size_to_cache(alloc_size)
                .alloc(&mut self.space_manager)
        };
        debug_assert!(
            ptr as usize == 0 || ptr as usize >= self.space_manager.alloc_area_start.as_usize(),
            "{:?} is invalid",
            ptr as usize
        );
        ptr
    }

    /// Deallocate.
    pub fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        debug_assert!(ptr as usize >= self.space_manager.alloc_area_start.as_usize());
        let alloc_size = HeapCaches::layout_to_size(layout);
        if alloc_size > HeapCaches::MAX_CACHE {
            self.space_manager
                .dealloc_big(Self::size_to_order(layout.size()), ptr)
        } else {
            self.caches
                .size_to_cache(alloc_size)
                .dealloc(&mut self.space_manager, ptr)
        }
    }
}

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.lock().as_mut().unchecked_unwrap().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner
            .lock()
            .as_mut()
            .unchecked_unwrap()
            .dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // TODO: could be more optimised for big reallocs if the size can be expanded in-place
        // Check if we need to reallocate. If the layouts map to the same cache, we don't.
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        if HeapCaches::layout_to_size(layout) == HeapCaches::layout_to_size(new_layout) {
            ptr
        } else {
            // This is the default implementation of realloc provided by the alloc library.
            let new_ptr = self.alloc(new_layout);
            if !new_ptr.is_null() {
                core::ptr::copy_nonoverlapping(ptr, new_ptr, cmp::min(layout.size(), new_size));
                self.dealloc(ptr, layout);
            }
            new_ptr
        }
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    // TODO: handle this better
    panic!("allocation error: {:?}", layout)
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap {
    inner: Spinlock::new(None),
};

/// Inits allocation. May only be called once.
pub unsafe fn init(reserved_end: VirtAddr) {
    assert!(ALLOCATOR.inner.lock().is_none());
    let heap = Heap::new(reserved_end);
    let max_end = heap.max_end();
    assert!(max_end.as_usize() < arch::TCB_START);
    assert!(max_end.as_usize() < arch::USER_START);
    *ALLOCATOR.inner.lock() = Some(heap);
}
