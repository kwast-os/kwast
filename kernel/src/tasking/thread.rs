use core::mem::size_of;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{EntryFlags, PAGE_SIZE};
use crate::arch::simd::SimdState;
use crate::arch::{preempt_disable, preempt_enable};
use crate::mm::mapper::MemoryError;
use crate::mm::vma_allocator::{LazilyMappedVma, MappableVma, MappedVma};
use crate::sync::spinlock::RwLock;
use crate::tasking::protection_domain::ProtectionDomain;
use crate::wasm::vmctx::{VmContextContainer, WASM_PAGE_SIZE};
use core::cell::Cell;

/// Stack size in bytes.
const STACK_SIZE: usize = 1024 * 256;

/// Amount of guard pages for stack underflow.
const AMOUNT_GUARD_PAGES: usize = 2;

/// The stack of a thread.
pub struct Stack {
    vma: Cell<MappedVma>,
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
    code: Cell<MappedVma>,
    id: ThreadId,
    _vmctx_container: Cell<Option<VmContextContainer>>,
    simd_state: SimdState,
    domain: ProtectionDomain,
}

impl Thread {
    /// Creates a thread.
    /// Unsafe because it's possible to set an entry point.
    pub unsafe fn create(
        domain: ProtectionDomain,
        entry: VirtAddr,
        first_arg: usize,
    ) -> Result<Thread, MemoryError> {
        // TODO: lazily allocate in the future?
        let stack_guard_size: usize = AMOUNT_GUARD_PAGES * PAGE_SIZE;
        let stack = {
            preempt_disable();
            let guard = domain.temporarily_switch();
            let mut stack = Stack::create(&domain, STACK_SIZE, stack_guard_size)?;
            stack.prepare_trampoline(entry, first_arg);
            drop(guard);
            preempt_enable();
            stack
        };
        Ok(Self::new(stack, domain))
    }

    /// Creates a new thread from given parameters.
    pub fn new(stack: Stack, domain: ProtectionDomain) -> Self {
        Self {
            stack,
            heap: RwLock::new(LazilyMappedVma::dummy()),
            code: Cell::new(MappedVma::dummy()),
            id: ThreadId::new(),
            _vmctx_container: Cell::new(None),
            domain,
            simd_state: SimdState::new(),
        }
    }

    /// Sets the thread wasm data.
    /// Unsafe when incorrect data is passed, or when used data is overwritten.
    pub unsafe fn set_wasm_data(
        &self,
        code_vma: MappedVma,
        heap_vma: LazilyMappedVma,
        vmctx_container: VmContextContainer,
    ) {
        self.code.set(code_vma);
        *self.heap.write() = heap_vma;
        self._vmctx_container.replace(Some(vmctx_container));
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

    /// Unmaps the memory that this thread holds.
    /// Unsafe because you can totally break memory mappings and safety if you call this
    /// while memory of this thread is still used somewhere.
    pub unsafe fn unmap_memory(&self) {
        self.domain.with(|vma, mapping| {
            let code = self.code.replace(MappedVma::dummy());
            vma.destroy_vma(mapping, &code);
            let stack = self.stack.vma.replace(MappedVma::dummy());
            vma.destroy_vma(mapping, &stack);
            let mut heap_guard = self.heap.write();
            vma.destroy_vma(mapping, &*heap_guard);
            *heap_guard = LazilyMappedVma::dummy();
        });
    }

    /// Gets the current protection domain.
    #[inline]
    pub fn domain(&self) -> &ProtectionDomain {
        &self.domain
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
        debug_assert_eq!(self.heap.get_mut().size(), 0);
        debug_assert_eq!(self.stack.vma.get_mut().size(), 0);
        debug_assert_eq!(self.code.get_mut().size(), 0);
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
            vma: Cell::new(vma),
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
        //debug_assert!(
        //    self.vma.get().is_dummy() || self.vma.get().is_contained(location),
        //    "the address {:?} does not belong to the thread's stack",
        //    location,
        //);
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
