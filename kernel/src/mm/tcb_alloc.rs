//! TCB allocation at a fixed space.
//! The goal is to provide a way to quickly map an id to a TCB reference.

use crate::arch::address::VirtAddr;
use crate::arch::paging::{ActiveMapping, EntryFlags, PAGE_SIZE};
use crate::arch::{TCB_LEN, TCB_START};
use crate::mm::mapper::MemoryMapper;
use crate::sync::spinlock::Spinlock;
use crate::tasking::thread::{Thread, ThreadId};
use crate::util::mem_funcs::page_clear;
use core::mem::{align_of, size_of, MaybeUninit};
use core::sync::atomic::{AtomicU16, Ordering};

static TCB_PAGE_LOCK: Spinlock<()> = Spinlock::new(());

struct Metadata {
    free: AtomicU16,
}

/// A TCB may be uninitialised.
/// We also want to align them on a cache line to minimise cache ping-pong.
/// The extra field is to reserve bytes for meta data.
#[repr(align(64))]
struct ThreadBlock(MaybeUninit<Thread>, Metadata);

/// How many TCBs fit inside on `TcbPage`?
const TCB_COUNT: usize = PAGE_SIZE / size_of::<ThreadBlock>();

struct TcbPage {
    threads: [ThreadBlock; TCB_COUNT],
}

impl TcbPage {
    /// Gets the metadata for this page.
    pub fn meta_data(&self) -> &Metadata {
        &self.threads[0].1
    }
}

const_assert!(size_of::<TcbPage>() <= PAGE_SIZE);
const_assert!(align_of::<TcbPage>() <= PAGE_SIZE);
const_assert!(TCB_COUNT <= 16);
const_assert_eq!(TCB_COUNT & (TCB_COUNT - 1), 0); // TCB_COUNT must be a power of two for efficiency

/// Pagefault TCB allocation handling.
pub fn pagefault_tcb_alloc(fault_addr: VirtAddr, write: bool) {
    assert!(write, "Attempting read to not existing thread");

    let _guard = TCB_PAGE_LOCK.lock();
    let fault_addr = fault_addr.align_down();

    // Safety:
    // No concurrent access on the shared page tables because these are unique for the TCB,
    // and we're locking.
    let mut mapping = unsafe { ActiveMapping::get_unlocked() };

    // Once we get this lock, the page might've been already mapped by another CPU.
    if mapping.translate(fault_addr).is_some() {
        return;
    }

    if mapping
        .get_and_map_single(
            fault_addr,
            EntryFlags::PRESENT | EntryFlags::NX | EntryFlags::WRITABLE | EntryFlags::GLOBAL,
        )
        .is_ok()
    {
        let ptr = fault_addr.as_const::<TcbPage>();
        // Clear out data
        unsafe {
            page_clear(ptr as *mut _);
        }
        // Safety: same as before.
        let tcb_page = unsafe { &*ptr };
        // Relaxed is fine since we're the ones initialising it, and others had to wait behind the lock.
        tcb_page.meta_data().free.store(u16::MAX, Ordering::Relaxed);
    } else {
        // TODO: OOM handling?
        unimplemented!();
    }
}

/// Converts a thread id to an address.
#[inline]
fn tid_to_addr(tid: ThreadId) -> (usize, usize) {
    let page_nr = (tid.as_u32() as usize) / TCB_COUNT;
    (
        TCB_START + page_nr * PAGE_SIZE,
        tid.as_u32() as usize % TCB_COUNT,
    )
}

/// Allocates a tcb.
pub fn tcb_alloc(thread: Thread) {
    let tid = thread.id;
    let (page_addr, offset) = tid_to_addr(tid);
    assert!(page_addr < TCB_START + TCB_LEN);
    // Safety:
    // The thread id is unique.
    // We only access the thread slot that is our own slot.
    // The issue is that we can't borrow the whole `TcbPage` or array as mutable.
    // So we have to borrow the slot as mutable using pointers.
    let page = unsafe {
        // Only non-mutable references are ever made to `TcbPage`.
        let page = &*(page_addr as *const TcbPage);
        let ptr = (&page.threads[offset].0) as *const _ as *mut MaybeUninit<Thread>;
        core::ptr::write(ptr, MaybeUninit::new(thread));
        page
    };
    page.meta_data()
        .free
        .fetch_xor((1 << offset) as _, Ordering::AcqRel);
}

/// Deallocates a tcb.
pub fn tcb_dealloc(tid: ThreadId) {
    // We want the deallocation to be under a lock to prevent racing in the pagefault handler.
    let _guard = TCB_PAGE_LOCK.lock();

    let (page_addr, offset) = tid_to_addr(tid);
    let page = unsafe { &*(page_addr as *const TcbPage) };
    let old_free = page
        .meta_data()
        .free
        .fetch_or((1 << offset) as _, Ordering::AcqRel);
    if old_free & (1 << offset) == 0 {
        // Safety: it was initialised and will be dropped only once.
        unsafe {
            // This will also invalidate the thread id.
            drop(page.threads[offset].0.assume_init_read());
        }
        if old_free | (1 << offset) == u16::MAX {
            // Safety:
            // No concurrent access on the shared page tables because these are unique for the TCB,
            // and we're locking.
            let mut mapping = unsafe { ActiveMapping::get_unlocked() };
            mapping.free_and_unmap_single(VirtAddr::new(page_addr));
        }
    }
}

/// Executes something in context of a thread.
///
/// # Panic
///
/// Panics if the thread is not valid anymore.
/// Callers should've been notified when a thread they're interested in ceases to exist.
pub fn with_thread<F, T>(tid: ThreadId, f: F) -> T
where
    F: FnOnce(&Thread) -> T,
{
    let (page_addr, offset) = tid_to_addr(tid);
    // Safety:
    // Only non-mutable references are ever made to `TcbPage`.
    let page = unsafe { &*(page_addr as *const TcbPage) };
    let block = &page.threads[offset];
    unsafe {
        // Safety:
        // We want to verify the thread id to be of the same generation.
        // We can't use `get_ref` on an uninitialized thread because that's undefined behaviour.
        // That means we have to read the field without calling `get_ref` first.
        // If there is no thread here, its id and generation will be zero.
        let tid_ptr =
            (block.0.as_ptr() as *const u8).add(offset_of!(Thread, id)) as *const ThreadId;
        assert_eq!(*tid_ptr, tid, "thread generation mismatch");

        // We now know that the thread was initialized, otherwise the assert would've failed.
        f(block.0.assume_init_ref())
    }
}
