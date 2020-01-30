use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::arch::address::VirtAddr;
use core::mem::size_of;
use crate::mm::mapper::MemoryMapper;
use crate::mm::buddy::{Tree, MAX_LEVEL};

/// Buddy test.
#[cfg(feature = "test-buddy")]
pub fn test_main() {
    let mut mapping = ActiveMapping::get();
    let addr: usize = 0xFFF00000;
    mapping.map_range(VirtAddr::new(addr), size_of::<Tree>(), EntryFlags::PRESENT | EntryFlags::WRITABLE).unwrap();
    let tree = unsafe { &mut *(addr as *mut Tree) };
    tree.init();

    assert_eq!(tree.alloc(3), Some(0));
    assert_eq!(tree.alloc(2), Some(8));
    assert_eq!(tree.alloc(3), Some(16));
    assert_eq!(tree.alloc(4), Some(32));
    assert_eq!(tree.alloc(2), Some(12));
    assert_eq!(tree.alloc(3), Some(24));
    assert_eq!(tree.alloc(6), Some(64));
    assert_eq!(tree.alloc(7), Some(128));
    assert_eq!(tree.alloc(MAX_LEVEL), None);

    assert_eq!(tree.alloc(3), Some(48));
    tree.dealloc(0);
    assert_eq!(tree.alloc(3), Some(0));
    tree.dealloc(48);
    assert_eq!(tree.alloc(3), Some(48));
    tree.dealloc(32);
    assert_eq!(tree.alloc(2), Some(32));
    assert_eq!(tree.alloc(2), Some(36));
    assert_eq!(tree.alloc(4), Some(256));
    assert_eq!(tree.alloc(2), Some(40));
    assert_eq!(tree.alloc(2), Some(44));
}