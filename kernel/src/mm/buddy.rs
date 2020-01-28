use core::mem::size_of;
use crate::arch::x86_64::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use crate::arch::x86_64::address::{VirtAddr, PhysAddr};

/// Amount of top nodes.
const TOP_NODE_COUNT: usize = 128;

/// Amount of bytes needed for top (see issue).
const L0_COUNT_BYTES: usize = TOP_NODE_COUNT / 2;
const L1_COUNT_BYTES: usize = L0_COUNT_BYTES * 2;
const L2_COUNT_BYTES: usize = L1_COUNT_BYTES * 2;
const L3_COUNT_BYTES: usize = L2_COUNT_BYTES * 2;
const L4_COUNT_BYTES: usize = L3_COUNT_BYTES * 2;
const L5_COUNT_BYTES: usize = L4_COUNT_BYTES * 2;
const L6_COUNT_BYTES: usize = L5_COUNT_BYTES * 2;
const L7_COUNT_BYTES: usize = L6_COUNT_BYTES * 2;
const L8_COUNT_BYTES: usize = L7_COUNT_BYTES * 2;
const L9_COUNT_BYTES: usize = L8_COUNT_BYTES * 2;
const L10_COUNT_BYTES: usize = L9_COUNT_BYTES * 2;
const L11_COUNT_BYTES: usize = L10_COUNT_BYTES * 2;
const L12_COUNT_BYTES: usize = L11_COUNT_BYTES * 2;

/// Nibble array.
// TODO: Once https://github.com/rust-lang/rust/issues/68567 is fixed, we can do the ugly
//       calculation in here.
struct NibbleArray<const N: usize> {
    /// Entries, stored as nibbles.
    entries: [u8; N],
}

impl<const N: usize> NibbleArray<N> {
    /// Gets the shift for the nibble.
    #[inline]
    fn get_shift(index: usize) -> u8 {
        ((index & 1) << 2) as u8
    }

    /// Gets a nibble at a logical index.
    fn get_nibble_at(&self, index: usize) -> u8 {
        (self.entries[index >> 1] >> NibbleArray::<N>::get_shift(index)) & 15
    }

    /// Sets a nibble at a logical index to a value.
    fn set_nibble_at(&mut self, index: usize, value: u8) {
        debug_assert!(value < 16);
        let shift = NibbleArray::<N>::get_shift(index);
        let masked = self.entries[index >> 1] & (0b11110000 >> shift);
        self.entries[index >> 1] = masked | (value << shift);
    }
}

/// The buddy tree.
struct Tree {
    /// Levels of the tree.
    l0: NibbleArray<L0_COUNT_BYTES>,
    l1: NibbleArray<L1_COUNT_BYTES>,
    l2: NibbleArray<L2_COUNT_BYTES>,
    l3: NibbleArray<L3_COUNT_BYTES>,
    l4: NibbleArray<L4_COUNT_BYTES>,
    l5: NibbleArray<L5_COUNT_BYTES>,
    l6: NibbleArray<L6_COUNT_BYTES>,
    l7: NibbleArray<L7_COUNT_BYTES>,
    l8: NibbleArray<L8_COUNT_BYTES>,
    l9: NibbleArray<L9_COUNT_BYTES>,
    l10: NibbleArray<L10_COUNT_BYTES>,
    l11: NibbleArray<L11_COUNT_BYTES>,
    l12: NibbleArray<L12_COUNT_BYTES>,
}

impl Tree {
    /// Construct new tree.
    pub fn new() {
        // TODO
    }

    pub fn test(&mut self) {
        // TODO
    }
}

#[allow(improper_ctypes)]
extern "C" {
    #[link_name = "llvm.x86.rdtscp"]
    fn rdtscp(aux: *mut u8) -> u64;
}

// TODO
pub fn test() {
    /*let mut x = Tree {
        l0: NibbleArray { entries: [0; 2 * TOP_NODE_COUNT / 4] },
        l1: NibbleArray { entries: [0; 4 * TOP_NODE_COUNT / 4] },
        l2: NibbleArray { entries: [0; 8 * TOP_NODE_COUNT / 4] },
        l3: NibbleArray { entries: [0; 16 * TOP_NODE_COUNT / 4] },
        l4: NibbleArray { entries: [0; 32 * TOP_NODE_COUNT / 4] },
        l5: NibbleArray { entries: [0; 64 * TOP_NODE_COUNT / 4] },
        l6: NibbleArray { entries: [0; 128 * TOP_NODE_COUNT / 4] },
        l7: NibbleArray { entries: [0; 256 * TOP_NODE_COUNT / 4] },
        l8: NibbleArray { entries: [0; 512 * TOP_NODE_COUNT / 4] },
        l9: NibbleArray { entries: [0; 1024 * TOP_NODE_COUNT / 4] },
        l10: NibbleArray { entries: [0; 2048 * TOP_NODE_COUNT / 4] },
        l11: NibbleArray { entries: [0; 4096 * TOP_NODE_COUNT / 4] },
        l12: NibbleArray { entries: [0; 8192 * TOP_NODE_COUNT / 4] },
    };*/

    unsafe {
        let mut xx: u8 = 0;

        println!("size: {:?}", size_of::<Tree>());
        println!("{} KiB", TOP_NODE_COUNT * 4096 * (1 << (12 - 1)) / 1024);

        let a = rdtscp(&mut xx);

        let lol = 0;
        let mut mapping = ActiveMapping::get();
        mapping.map_range(VirtAddr::new(0xFC00000), PhysAddr::new(0xFC00000), 4096 * 8, EntryFlags::PRESENT | EntryFlags::WRITABLE);

        let b = rdtscp(&mut xx);

        println!("{}ns", ((b - a - 5) * 284) >> 10);
        println!("res: {}", lol);
    }
}
