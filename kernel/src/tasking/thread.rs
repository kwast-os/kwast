use core::mem::size_of;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{
    get_cpu_page_mapping, ActiveMapping, CpuPageMapping, EntryFlags, PAGE_SIZE,
};
use crate::arch::simd::SimdState;
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::mm::vma_allocator::{LazilyMappedVma, MappableVma, MappedVma, VmaAllocator};
use crate::sync::spinlock::{RwLock, Spinlock};
use crate::wasm::vmctx::{VmContextContainer, WASM_PAGE_SIZE};
use alloc::sync::Arc;
use core::borrow::BorrowMut;
use core::cell::Cell;
use core::ops::DerefMut;

/// Stack size in bytes.
const STACK_SIZE: usize = 1024 * 256;

/// Amount of guard pages for stack underflow.
const AMOUNT_GUARD_PAGES: usize = 2;

/// Hardware memory protection domain.
/// Responsible for safely getting both an active mapping & getting an address allocator.
pub struct ProtectionDomain(Arc<Spinlock<ProtectionDomainInner>>);

/// Inner structure of a ProtectionDomain.
struct ProtectionDomainInner {
    vma_allocator: VmaAllocator,
    mapping: ActiveMapping,
}

/// The stack of a thread.
#[derive(Debug)]
pub struct Stack {
    vma: MappedVma,
    current_location: Cell<VirtAddr>,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ThreadId(u64);

impl ProtectionDomain {
    /// Creates a new protection domain.
    pub fn new() -> Self {
        Self(Arc::new(Spinlock::new(ProtectionDomainInner {
            vma_allocator: VmaAllocator::new(),
            mapping: unsafe { ActiveMapping::get_unlocked() },
        })))
    }

    /// Checks if we can avoid locks for this domain.
    #[inline]
    fn can_avoid_locks(&self) -> bool {
        // We can avoid locks if we have only one thread containing this domain.
        // That's because to clone this domain, you need to have access to a thread which
        // has access to this domain.
        // Since this code is also executing from a thread containing this domain,
        // we know that this is the only executing code that has access to this domain.
        // That means we can avoid locking because this thread is the only accessor.
        Arc::strong_count(&self.0) == 1
    }

    /// Clones this domain reference.
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    /// Execute action with both the Vma allocator and active mapping.
    #[inline]
    pub fn with<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut VmaAllocator, &mut ActiveMapping) -> T,
    {
        if self.can_avoid_locks() {
            let inner = unsafe { &mut *self.0.get_cell().get() };
            f(&mut inner.vma_allocator, &mut inner.mapping)
        } else {
            let mut inner = self.0.lock();
            let inner = inner.deref_mut();
            f(&mut inner.vma_allocator, &mut inner.mapping)
        }
    }
}

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
    pub cpu_page_mapping: CpuPageMapping,
    heap: RwLock<LazilyMappedVma>, // TODO: Something lighter? since this is only an issue with shared heaps
    code: MappedVma,
    id: ThreadId,
    _vmctx_container: Option<VmContextContainer>,
    simd_state: SimdState,
    domain: ProtectionDomain,
}

impl Thread {
    /// Creates a thread.
    /// Unsafe because it's possible to set an entry point.
    pub unsafe fn create(
        domain: ProtectionDomain,
        entry: VirtAddr,
        code: MappedVma,
        heap: LazilyMappedVma,
        vmctx_container: VmContextContainer,
    ) -> Result<Thread, MemoryError> {
        println!("DLJIFMSLDJFMLKSDJFMLKSDJF");
        // TODO: lazily allocate in the future?
        let stack_guard_size: usize = AMOUNT_GUARD_PAGES * PAGE_SIZE;
        let mut stack = Stack::create(&domain, STACK_SIZE, stack_guard_size)?;
        println!("DLJIFMSLDJFMLKSDJFMLKSDJF");
        // Safe because enough size on the stack and memory allocated at a known good location.
        stack.prepare_trampoline(entry, vmctx_container.ptr());
        println!("DLJIFMSLDJFMLKSDJFMLKSDJF");
        Ok(Self::new(stack, code, heap, domain, Some(vmctx_container)))
    }

    /// Creates a new thread from given parameters.
    pub fn new(
        stack: Stack,
        code: MappedVma,
        heap: LazilyMappedVma,
        domain: ProtectionDomain,
        vmctx_container: Option<VmContextContainer>,
    ) -> Self {
        Self {
            cpu_page_mapping: get_cpu_page_mapping(),
            stack,
            heap: RwLock::new(heap),
            code,
            id: ThreadId::new(),
            _vmctx_container: vmctx_container,
            domain,
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
        self.domain
            .with(|_vma, mapping| self.heap.write().try_handle_page_fault(mapping, fault_addr))
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

impl Drop for Thread {
    fn drop(&mut self) {
        // TODO: free PML4

        let code = self.code.borrow_mut();
        let stack = self.stack.vma.borrow_mut();
        let heap = self.heap.get_mut();

        self.domain.with(|vma, mapping| {
            vma.destroy_vma(mapping, code);
            vma.destroy_vma(mapping, heap);
            vma.destroy_vma(mapping, stack);
        });
    }
}

impl Stack {
    /// Creates a stack.
    pub fn create(
        domain: &ProtectionDomain,
        size: usize,
        guard_size: usize,
    ) -> Result<Stack, MemoryError> {
        let vma = {
            let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;

            domain.with(|vma, mapping| {
                vma.create_vma(size + guard_size)?
                    .map(mapping, guard_size, size, flags)
            })?
        };
        Ok(Stack::new(vma))
    }

    /// Creates a new stack from given parameters.
    pub fn new(vma: MappedVma) -> Self {
        let current_location = vma.address() + vma.size();
        Self {
            vma,
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
            self.vma.is_dummy() || self.vma.is_contained(location),
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
