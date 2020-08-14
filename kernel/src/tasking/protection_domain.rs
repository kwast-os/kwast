use crate::arch::asid::Asid;
use crate::arch::paging::{
    cpu_page_mapping_switch_to, get_cpu_page_mapping, ActiveMapping, CpuPageMapping, EntryFlags,
    PAGE_SIZE,
};
use crate::arch::{get_per_cpu_data, preempt_disable, preempt_enable};
use crate::mm::mapper::{MemoryError, MemoryMapper};
use crate::mm::vma_allocator::VmaAllocator;
use crate::sync::spinlock::Spinlock;
use crate::tasking::scheduler::with_current_thread;
use alloc::sync::Arc;
use atomic::Atomic;
use core::ops::DerefMut;

/// Hardware memory protection domain.
/// Responsible for safely getting both an active mapping & getting an address allocator.
pub struct ProtectionDomain(Arc<ProtectionDomainInner>);

/// Inner structure of a ProtectionDomain.
struct ProtectionDomainInner {
    vma_allocator: Spinlock<VmaAllocator>,
    mapping: CpuPageMapping,
    asid: bool,
    // TODO: expand on multi-core systems
    current_asid: Atomic<Asid>,
}

/// Temporary switch guard. Returns to old page mapping when dropped.
pub struct SwitchGuard(CpuPageMapping);

impl SwitchGuard {
    /// Creates a new switch guard for a new mapping.
    unsafe fn new(new_mapping: CpuPageMapping) -> Self {
        preempt_disable();
        let result = Self(get_cpu_page_mapping());
        cpu_page_mapping_switch_to(new_mapping);
        result
    }
}

impl Drop for SwitchGuard {
    fn drop(&mut self) {
        unsafe {
            cpu_page_mapping_switch_to(self.0);
        }
        preempt_enable();
    }
}

impl ProtectionDomain {
    /// Creates a new protection domain.
    pub fn new() -> Result<Self, MemoryError> {
        Ok(unsafe { Self::from_existing_mapping(ActiveMapping::get_new()?) })
    }

    /// Creates a domain from an existing cpu page mapping.
    pub unsafe fn from_existing_mapping(mapping: CpuPageMapping) -> Self {
        let (current_asid, asid) = if let Some(asid_manager) = get_per_cpu_data().asid_manager() {
            (asid_manager.borrow_mut().alloc(Asid::null()), true)
        } else {
            (Asid::null(), false)
        };

        Self(Arc::new(ProtectionDomainInner {
            vma_allocator: Spinlock::new(VmaAllocator::new()),
            mapping,
            asid,
            current_asid: Atomic::new(current_asid),
        }))
    }

    /// Assign asid if necessary
    pub fn assign_asid_if_necessary(&self) {
        if let Some(asid_manager) = get_per_cpu_data().asid_manager() {
            let mut asid_manager = asid_manager.borrow_mut();
            let old = self.0.current_asid.load(atomic::Ordering::Acquire);
            if !asid_manager.is_valid(old) {
                self.0
                    .current_asid
                    .store(asid_manager.alloc(old), atomic::Ordering::Release);
            }
        }
    }

    /// Temporarily switch to this mapping.
    pub unsafe fn temporarily_switch(&self) -> SwitchGuard {
        SwitchGuard::new(self.0.mapping)
    }

    /// Gets the cpu page mapping
    #[inline]
    pub fn cpu_page_mapping(&self) -> CpuPageMapping {
        if self.0.asid {
            self.0
                .mapping
                .with_asid(self.0.current_asid.load(atomic::Ordering::Acquire))
        } else {
            self.0.mapping
        }
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
    pub fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    /// Execute action with both the Vma allocator and active mapping.
    #[inline]
    pub fn with<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut VmaAllocator, &mut ActiveMapping) -> T,
    {
        debug_assert_eq!(
            self.0.mapping.as_phys_addr(),
            get_cpu_page_mapping().as_phys_addr()
        );

        if self.can_avoid_locks() {
            let inner = unsafe { &mut *self.0.vma_allocator.get_cell().get() };
            // Safety: only we can access the active mapping, since we're the only thread.
            //         Otherwise can_avoid_locks would've returned false.
            f(inner, &mut unsafe { ActiveMapping::get_unlocked() })
        } else {
            let mut inner = self.0.vma_allocator.lock();
            let inner = inner.deref_mut();
            // Safety: inner lock also covers the active mapping.
            f(inner, &mut unsafe { ActiveMapping::get_unlocked() })
        }
    }
}

impl Drop for ProtectionDomainInner {
    fn drop(&mut self) {
        debug_assert_ne!(self.mapping, get_cpu_page_mapping());

        // Free the old asid.
        if let Some(asid_manager) = get_per_cpu_data().asid_manager() {
            asid_manager
                .borrow_mut()
                .free(self.current_asid.load(atomic::Ordering::Acquire));
        }

        // The PMM expects a virtual address because it needs to update the list.
        // We can use the mapping system to map a page without allocating a frame,
        // and then unmapping _with_ freeing the frame.
        with_current_thread(|thread| {
            thread.domain().with(|vma, mapping| {
                let paddr = self.mapping.as_phys_addr();

                let _ = vma
                    .alloc_region(PAGE_SIZE)
                    .ok_or(MemoryError::NoMoreVMA)
                    .and_then(|vaddr| {
                        mapping.map_range_physical(
                            vaddr,
                            paddr,
                            PAGE_SIZE,
                            EntryFlags::PRESENT | EntryFlags::NX | EntryFlags::WRITABLE,
                        )?;
                        Ok(vaddr)
                    })
                    .map(|vaddr| mapping.unmap_single(vaddr));
            });
        });
    }
}
