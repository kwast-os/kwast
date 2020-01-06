use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging;

use super::mapper::{MappingError, MappingResult};

/// Frame allocator trait.
pub trait FrameAllocator {
    /// Gets a physical address.
    fn alloc(&mut self) -> Option<PhysAddr>;
}

/// Helper, used before the default frame allocator is available.
pub struct IteratorFrameAllocator<I>
    where I: Iterator<Item=PhysAddr> {
    frames: I,
}

impl<I> IteratorFrameAllocator<I>
    where I: Iterator<Item=PhysAddr> {
    pub fn new(frames: I) -> Self {
        Self {
            frames,
        }
    }

    pub fn unwrap_frames_iter(self) -> I {
        self.frames
    }
}

impl<I> FrameAllocator for IteratorFrameAllocator<I>
    where I: Iterator<Item=PhysAddr> {
    fn alloc(&mut self) -> Option<PhysAddr> {
        self.frames.next()
    }
}

/// The default frame allocator. Can be used after physical map is ready.
///
/// How does this allocator work?
/// Instead of having a fixed area in the memory to keep the stack,
/// we let each free frame contain a pointer to the next free frame on the stack.
/// This limits the amount of virtual memory we need to reserve.
///
/// When we allocate a frame, we map it to the virtual memory and read the pointer.
/// Then we move the head. There is no unnecessary mapping happening here.
/// There is no additional mapping compared to the classical stack approach:
/// * When a page is being allocated it'll need to be mapped anyway.
/// * When a page is being freed it was already mapped.
///
/// It is likely that, for an allocation, the data will be accessed anyway after the mapping.
/// For a free, it is likely that the data was already accessed.
#[derive(Debug)]
pub struct DefaultFrameAllocator {
    top: PhysAddr,
    test: AtomicUsize,
}

impl DefaultFrameAllocator {
    /// Inits the memory manager.
    pub fn init<I>(&mut self, it: &mut I)
        where I: Iterator<Item=PhysAddr> {
        self.top = self.init_internal(it);
    }

    /// Empty, uninitialized allocator.
    const fn empty() -> Self {
        DefaultFrameAllocator {
            top: PhysAddr::null(),
            test: AtomicUsize::new(0),
        }
    }

    /// Debug print all frames.
    #[allow(dead_code)]
    fn debug_print_frames(&self) {
        println!("debug print frames");

        let mut top = self.top.as_usize();
        while top != 0 {
            print!("{:x} ", top / paging::PAGE_SIZE);
            top = unsafe { *((top | paging::PHYS_OFF) as *mut usize) };
        }

        println!();
    }
}

impl FrameAllocator for DefaultFrameAllocator {
    fn alloc(&mut self) -> Option<PhysAddr> {
        // TODO: ABA
        // TODO: top null

        //self.top.load(Ordering::Acquire);

        unimplemented!()
    }
}

static mut PMM: DefaultFrameAllocator = DefaultFrameAllocator::empty();

/// Gets the PMM.
pub fn get() -> &'static mut DefaultFrameAllocator {
    // PMM is lock-free. Multiple threads accessing this won't cause harm.
    unsafe { &mut PMM }
}
