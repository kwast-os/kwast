use core::mem::size_of;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{EntryFlags, PAGE_SIZE, ActiveMapping};
use crate::arch::simd::SimdState;
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::mm::vma_allocator::{LazilyMappedVma, MappableVma, MappedVma, Vma, VmaAllocator};
use crate::sync::spinlock::RwLock;
use crate::wasm::vmctx::{VmContextContainer, WASM_PAGE_SIZE};
use core::cell::Cell;

/// Stack size in bytes.
const STACK_SIZE: usize = 1024 * 256;

/// Amount of guard pages for stack underflow.
const AMOUNT_GUARD_PAGES: usize = 2;

/// The stack of a thread.
#[derive(Debug)]
pub struct Stack {
    _vma: MappedVma,
    current_location: Cell<VirtAddr>,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ThreadId(u64);

impl ThreadId {
    /// Create new thread id.
    pub fn new() -> Self {
        use core::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::SeqCst))
    }
}

pub struct Thread {
    pub stack: Stack,
    heap: RwLock<LazilyMappedVma>, // TODO: Something lighter? since this is only an issue with shared heaps
    _code: MappedVma,
    id: ThreadId,
    _vmctx_container: Option<VmContextContainer>,
    simd_state: SimdState,
}

impl Thread {
    /// Creates a thread.
    /// Unsafe because it's possible to set an entry point.
    pub unsafe fn create(
        mut vma_allocator: VmaAllocator,
        entry: VirtAddr,
        code: MappedVma,
        heap: LazilyMappedVma,
        vmctx_container: VmContextContainer,
    ) -> Result<Thread, MemoryError> {
        // TODO: lazily allocate in the future?
        let stack_guard_size: usize = AMOUNT_GUARD_PAGES * PAGE_SIZE;
        let mut stack = Stack::create(&mut vma_allocator, STACK_SIZE, stack_guard_size)?;
        // Safe because enough size on the stack and memory allocated at a known good location.
        stack.prepare_trampoline(entry, vmctx_container.ptr());
        Ok(Self::new(stack, code, heap, Some(vmctx_container)))
    }

    /// Creates a new thread from given parameters.
    pub fn new(
        stack: Stack,
        code: MappedVma,
        heap: LazilyMappedVma,
        vmctx_container: Option<VmContextContainer>,
    ) -> Self {
        Self {
            stack,
            heap: RwLock::new(heap),
            _code: code,
            id: ThreadId::new(),
            _vmctx_container: vmctx_container,
            simd_state: SimdState::new(),
        }
    }

    /// Gets the thread id.
    #[inline]
    pub fn id(&self) -> ThreadId {
        self.id
    }

    /// Gets the current allocated heap size in WebAssembly pages.
    pub fn heap_size(&self) -> usize {
        self.heap.read().size()
    }

    /// Grows the heap by `wasm_pages` WebAssembly pages.
    pub fn heap_grow(&self, wasm_pages: u32) -> u32 {
        self.heap
            .write()
            .expand((wasm_pages as usize) * WASM_PAGE_SIZE)
            .map_or(core::u32::MAX, |x| (x / WASM_PAGE_SIZE) as u32)
    }

    /// Handle a page fault for this thread. Returns true if handled successfully.
    #[inline]
    pub fn page_fault(&self, fault_addr: VirtAddr) -> bool {
        // TODO: should be locked if needed
        let mut mapping = unsafe { ActiveMapping::get_unlocked() };

        self.heap.write().try_handle_page_fault(&mut mapping, fault_addr)
    }

    /// Save SIMD state.
    #[inline]
    pub fn save_simd(&self) {
        self.simd_state.save();
    }

    /// Restore SIMD state.
    #[inline]
    pub fn restore_simd(&self) {
        self.simd_state.restore();
    }
}

impl Stack {
    /// Creates a stack.
    pub fn create(vma_allocator: &mut VmaAllocator, size: usize, guard_size: usize) -> Result<Stack, MemoryError> {
        let vma = {
            let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;

            // TODO: should be locked if needed
            let mut mapping = unsafe { ActiveMapping::get_unlocked() };

            vma_allocator.create_vma(size + guard_size)?.map(&mut mapping, guard_size, size, flags)?
        };
        Ok(Stack::new(vma))
    }

    /// Creates a new stack from given parameters.
    pub fn new(vma: MappedVma) -> Self {
        let current_location = vma.address() + vma.size();
        Self {
            _vma: vma,
            current_location: Cell::new(current_location),
        }
    }

    /// Gets the current location.
    #[inline]
    pub fn get_current_location(&self) -> VirtAddr {
        self.current_location.get()
    }

    /// Sets the current location.
    #[inline]
    pub fn set_current_location(&self, location: VirtAddr) {
        debug_assert!(
            self._vma.is_dummy() || self._vma.is_contained(location),
            "the address {:?} does not belong to the stack {:?}",
            location,
            self
        );
        self.current_location.replace(location);
    }

    /// Pushes a value on the stack.
    pub unsafe fn push<T>(&mut self, value: T) {
        let current = self.current_location.get_mut();
        *current -= size_of::<T>();
        let ptr = current.as_mut();
        *ptr = value;
    }
}
