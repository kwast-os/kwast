use crate::arch::address::VirtAddr;
use core::cmp;
use core::intrinsics::unlikely;
use core::mem::MaybeUninit;

/// Amount of top nodes.
pub const MAX_LEVEL: usize = 18; // TODO: choose a good size?

/// Amount of nodes.
pub const NODE_COUNT: usize = (1 << MAX_LEVEL) - 1;

/// Amount of bytes needed.
const NODE_BYTES_NEEDED: usize = NODE_COUNT;

/// Max offset.
pub const MAX_OFFSET: usize = (1 << (MAX_LEVEL - 1)) - 1;

/// Tree entries.
type Entries = [u8; NODE_BYTES_NEEDED];
type MaybeUninitEntries = [MaybeUninit<u8>; NODE_BYTES_NEEDED];

/// The buddy tree.
#[repr(transparent)]
pub struct Tree(Entries);

impl Tree {
    /// Initializes the tree.
    /// This is unsafe as we can't verify that the `tree_location` is valid
    /// and will live long enough.
    pub(crate) unsafe fn from(tree_location: VirtAddr) -> &'static mut Self {
        // Is power of 2? We don't care about the case of x == 0 here.
        fn is_pow2(x: usize) -> bool {
            x & (x - 1) == 0
        }

        // Limit scope of unsafety, this procedure is safe.
        fn fill_nodes(entries: &mut MaybeUninitEntries) {
            let mut size = (MAX_LEVEL + 1) as u8;
            for (i, entry) in entries.iter_mut().enumerate() {
                if is_pow2(i + 1) {
                    size -= 1;
                }

                *entry = MaybeUninit::new(size);
            }
        }

        // Safety:
        // Assumptions caller must guarantee are in the method docs.
        let array = &mut *(tree_location.as_mut::<MaybeUninitEntries>());
        // This is safe.
        fill_nodes(array);
        // Safety:
        // `MaybeUninit<u8>` and `u8` have the same ABI, size & alignment.
        // Thus, `Entries` and `MaybeUninitEntries` may be transmuted to each other.
        // `Tree` and `Entries` have the same ABI, size & alignment due to the repr(transparent).
        &mut *(array as *mut _ as *mut Tree)
    }

    /// Left index of a node.
    #[inline]
    fn left_index(&self, index: usize) -> usize {
        (index << 1) | 1
    }

    /// Right index of a node.
    #[inline]
    fn right_index(&self, index: usize) -> usize {
        self.left_index(index) + 1
    }

    /// Parent index of a node.
    #[inline]
    fn parent_index(&self, index: usize) -> usize {
        ((index + 1) >> 1) - 1
    }

    /// Allocate in tree.
    pub fn alloc(&mut self, order: usize) -> Option<usize> {
        if unlikely(self.0[0] < 1 + order as u8) {
            return None;
        }

        // Find node with smallest size large enough to hold the requested size
        let wanted_level = MAX_LEVEL - 1 - order;
        let mut index = 0;
        for _ in 0..wanted_level {
            let left_index = self.left_index(index);
            let right_index = self.right_index(index);

            // Because of the check at the beginning, we know one of these two is big enough
            index = if self.0[left_index] > order as u8 {
                left_index
            } else {
                debug_assert!(self.0[right_index] > order as u8);
                right_index
            };
        }

        // Calculate offset from the index
        let first_index_in_this_level = (1 << wanted_level) - 1;
        let index_in_this_level = index - first_index_in_this_level;
        let offset = index_in_this_level << order;

        // Update the values in the tree so that each node still contains the largest available
        // power of two size in their subtree.
        self.0[index] = 0;
        while index > 0 {
            index = self.parent_index(index);
            let left_index = self.left_index(index);
            let right_index = self.right_index(index);
            let max = cmp::max(self.0[left_index], self.0[right_index]);
            self.0[index] = max;
        }

        Some(offset)
    }

    // Deallocate in tree.
    pub fn dealloc(&mut self, order: usize, offset: usize) {
        // Calculate the index at which this allocation happened.
        let mut size = (order + 1) as u8;
        let wanted_level = MAX_LEVEL - size as usize;
        let index_in_this_level = offset >> order;
        let first_index_in_this_level = (1 << wanted_level) - 1;
        let mut index = index_in_this_level + first_index_in_this_level;

        // Update value in the tree to undo the allocation.
        debug_assert_eq!(self.0[index], 0);
        self.0[index] = size;

        // Update all parents in the tree.
        while index > 0 {
            index = self.parent_index(index);
            size += 1;

            let left_index = self.left_index(index);
            let right_index = self.right_index(index);

            // This node becomes a complete node again if both the children are complete nodes.
            self.0[index] =
                if self.0[left_index] == self.0[right_index] && self.0[left_index] == size - 1 {
                    size
                } else {
                    cmp::max(self.0[left_index], self.0[right_index])
                };
        }
    }
}
