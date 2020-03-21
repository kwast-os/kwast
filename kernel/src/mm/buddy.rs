use core::cmp;
use core::intrinsics::unlikely;

/// Amount of top nodes.
pub const MAX_LEVEL: usize = 18; // TODO: choose a good size?

/// Amount of nodes.
pub const NODE_COUNT: usize = (1 << MAX_LEVEL) - 1;

/// Amount of bytes needed.
const NODE_BYTES_NEEDED: usize = NODE_COUNT;

/// Max offset.
pub const MAX_OFFSET: usize = (1 << (MAX_LEVEL - 1)) - 1;

/// The buddy tree.
pub struct Tree {
    /// Entries in the tree.
    nodes: [u8; NODE_BYTES_NEEDED],
}

impl Tree {
    /// Initializes the tree.
    pub fn init(&mut self) {
        // Is power of 2? We don't care about the case of x == 0 here.
        fn is_pow2(x: usize) -> bool {
            x & (x - 1) == 0
        }

        let mut size = (MAX_LEVEL + 1) as u8;
        for i in 0..NODE_COUNT {
            if is_pow2(i + 1) {
                size -= 1;
            }

            self.nodes[i] = size;
        }
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
        if unlikely(self.nodes[0] < 1 + order as u8) {
            return None;
        }

        // Find node with smallest size large enough to hold the requested size
        let wanted_level = MAX_LEVEL - 1 - order;
        let mut index = 0;
        for _ in 0..wanted_level {
            let left_index = self.left_index(index);
            let right_index = self.right_index(index);

            // Because of the check at the beginning, we know one of these two is big enough
            index = if self.nodes[left_index] > order as u8 {
                left_index
            } else {
                debug_assert!(self.nodes[right_index] > order as u8);
                right_index
            };
        }

        // Calculate offset from the index
        let first_index_in_this_level = (1 << wanted_level) - 1;
        let index_in_this_level = index - first_index_in_this_level;
        let offset = index_in_this_level << order;

        // Update the values in the tree so that each node still contains the largest available
        // power of two size in their subtree.
        self.nodes[index] = 0;
        while index > 0 {
            index = self.parent_index(index);
            let left_index = self.left_index(index);
            let right_index = self.right_index(index);
            let max = cmp::max(self.nodes[left_index], self.nodes[right_index]);
            self.nodes[index] = max;
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
        debug_assert_eq!(self.nodes[index], 0);
        self.nodes[index] = size;

        // Update all parents in the tree.
        while index > 0 {
            index = self.parent_index(index);
            size += 1;

            let left_index = self.left_index(index);
            let right_index = self.right_index(index);

            // This node becomes a complete node again if both the children are complete nodes.
            self.nodes[index] = if self.nodes[left_index] == self.nodes[right_index]
                && self.nodes[left_index] == size - 1
            {
                size
            } else {
                cmp::max(self.nodes[left_index], self.nodes[right_index])
            };
        }
    }
}
