use core::mem::size_of;

use crate::arch::address::VirtAddr;
use crate::arch::paging::{ActiveMapping, EntryFlags, PAGE_SIZE};
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::mm::vma_allocator::LazilyMappedVma;
use crate::mm::vma_allocator::MappableVma;
use crate::mm::vma_allocator::{MappedVma, Vma};
use crate::wasm::vmctx::VmContextContainer;
use bitflags::_core::intrinsics::write_bytes;
use core::cell::Cell;
use core::intrinsics::likely;

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
    heap: LazilyMappedVma,
    _code: MappedVma,
    id: ThreadId,
    _vmctx_container: Option<VmContextContainer>,
}

impl Thread {
    /// Creates a thread.
    pub fn create(
        entry: VirtAddr,
        code: MappedVma,
        heap: LazilyMappedVma,
        vmctx_container: VmContextContainer,
    ) -> Result<Thread, MemoryError> {
        // TODO
        let stack_size = 8 * PAGE_SIZE;
        let stack_guard_size: usize = PAGE_SIZE;
        let mut stack = Stack::create(stack_size, stack_guard_size)?;
        // Safe because enough size on the stack and stack allocated at a known good location.
        unsafe {
            stack.prepare_trampoline(entry, vmctx_container.ptr());
            Ok(Self::new(stack, code, heap, Some(vmctx_container)))
        }
    }

    /// Creates a new thread from given parameters.
    pub unsafe fn new(
        stack: Stack,
        code: MappedVma,
        heap: LazilyMappedVma,
        vmctx_container: Option<VmContextContainer>,
    ) -> Self {
        Self {
            stack,
            heap,
            _code: code,
            id: ThreadId::new(),
            _vmctx_container: vmctx_container,
        }
    }

    /// Gets the thread id.
    pub fn id(&self) -> ThreadId {
        self.id
    }

    /// Handle a page fault for this thread. Returns true if handled successfully.
    pub fn page_fault(&self, fault_addr: VirtAddr) -> bool {
        // Optimize for the likely case.
        if likely(self.heap.is_contained(fault_addr)) {
            let mut mapping = ActiveMapping::get();
            let flags = self.heap.flags();

            // After the mapping is successful, we need to clear the memory to avoid information leaks.
            if mapping
                .get_and_map_single(fault_addr.align_down(), flags)
                .is_ok()
            {
                let ptr: *mut u8 = fault_addr.as_mut();
                // Safe because valid pointer and valid size.
                unsafe {
                    write_bytes(ptr, 0, PAGE_SIZE);
                }

                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl Stack {
    /// Creates a stack.
    pub fn create(size: usize, guard_size: usize) -> Result<Stack, MemoryError> {
        let vma = {
            let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX;
            Vma::create(size + guard_size)?.map(guard_size, size, flags)?
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
