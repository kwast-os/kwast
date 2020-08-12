use crate::arch::address::VirtAddr;
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::buddy::{Tree, MAX_LEVEL};
use crate::mm::mapper::MemoryMapper;
use core::mem::size_of;

/// Buddy test.
#[cfg(feature = "test-buddy")]
pub fn test_main() {
    let mut mapping = unsafe { ActiveMapping::get_unlocked() };
    let addr = VirtAddr::new(0xFFF00000);
    mapping
        .map_range(
            addr,
            size_of::<Tree>(),
            EntryFlags::PRESENT | EntryFlags::WRITABLE,
        )
        .unwrap();
    let tree = unsafe { Tree::from(addr) };

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
    tree.dealloc(3, 0);
    assert_eq!(tree.alloc(3), Some(0));
    tree.dealloc(3, 48);
    assert_eq!(tree.alloc(3), Some(48));
    tree.dealloc(4, 32);
    assert_eq!(tree.alloc(2), Some(32));
    assert_eq!(tree.alloc(2), Some(36));
    assert_eq!(tree.alloc(4), Some(256));
    assert_eq!(tree.alloc(2), Some(40));
    assert_eq!(tree.alloc(2), Some(44));
}
