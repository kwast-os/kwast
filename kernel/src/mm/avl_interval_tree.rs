use alloc::boxed::Box;
use core::cmp;
use core::cmp::Ordering;

type TreeNode = Option<Box<Node>>;

#[derive(Debug, Eq, PartialEq)]
struct Node {
    interval_start: usize,
    interval_len: usize,
    left: TreeNode,
    right: TreeNode,
    max_len: usize,
    height: u8,
}

impl Node {
    /// Safe height getter.
    fn height(n: &TreeNode) -> u8 {
        n.as_ref().map_or(0, |n| n.height)
    }

    /// Safe max_len getter.
    fn max_len(n: &TreeNode) -> usize {
        n.as_ref().map_or(0, |n| n.max_len)
    }

    /// Calculates balance factor.
    fn balance_factor(&self) -> isize {
        let lh = Self::height(&self.left) as isize;
        let rh = Self::height(&self.right) as isize;
        lh - rh
    }

    /// Update height.
    fn update_height(&mut self) {
        self.height = 1 + cmp::max(Self::height(&self.left), Self::height(&self.right));
    }

    /// Update max length.
    fn update_max_len(&mut self) {
        self.max_len = cmp::max(
            self.interval_len,
            cmp::max(Self::max_len(&self.left), Self::max_len(&self.right)),
        );
    }

    /// Update additional data fields: as height and max length.
    fn update_fields(&mut self) {
        self.update_height();
        self.update_max_len();
    }

    /// Left rotation. Returns the new root.
    fn rotate_left(mut root: Box<Node>) -> Box<Node> {
        let mut new_root = root.right.take().unwrap();
        root.right = new_root.left.take();
        root.update_fields();
        new_root.left = Some(root);
        new_root.update_fields();
        new_root
    }

    /// Right rotation. Returns the new root.
    fn rotate_right(mut root: Box<Node>) -> Box<Node> {
        let mut new_root = root.left.take().unwrap();
        root.left = new_root.right.take();
        root.update_fields();
        new_root.right = Some(root);
        new_root.update_fields();
        new_root
    }

    /// Fixes the AVL rotations. Returns the new root.
    fn fixup(mut root: Box<Node>) -> Box<Node> {
        // Calculate the balance factor and check if we need rebalancing.
        let balance_factor = root.balance_factor();
        if balance_factor == 2 {
            // Subtree is left heavy.

            // Must exist, otherwise this balance factor is impossible.
            let left = root.left.take().expect("left heavy");
            if left.balance_factor() == -1 {
                root.left = Some(Self::rotate_left(left));
            } else {
                root.left = Some(left);
            }

            Self::rotate_right(root)
        } else if balance_factor == -2 {
            // Subtree is right heavy.

            // Must exist, otherwise this balance factor is impossible.
            let right = root.right.take().expect("right heavy");
            if right.balance_factor() == 1 {
                root.right = Some(Self::rotate_right(right));
            } else {
                root.right = Some(right);
            }

            Self::rotate_left(root)
        } else {
            // Fields are not updated by rotations, update it manually here.
            root.update_fields();
            root
        }
    }
}

#[derive(Debug)]
pub struct AVLIntervalTree {
    root: TreeNode,
}

impl AVLIntervalTree {
    /// Constructs an empty AVL interval tree.
    pub const fn new() -> Self {
        Self { root: None }
    }

    /// Insert helper. Returns new root.
    fn insert_helper(root: TreeNode, interval_start: usize, interval_len: usize) -> TreeNode {
        match root {
            None => Some(Box::new(Node {
                interval_start,
                interval_len,
                max_len: interval_len,
                left: None,
                right: None,
                height: 1,
            })),
            Some(mut root) => {
                if interval_start < root.interval_start {
                    root.left = Self::insert_helper(root.left.take(), interval_start, interval_len);
                } else {
                    root.right =
                        Self::insert_helper(root.right.take(), interval_start, interval_len);
                }

                Some(Node::fixup(root))
            }
        }
    }

    /// Inserts a new interval in the tree.
    pub fn insert(&mut self, interval_start: usize, interval_len: usize) {
        self.root = Self::insert_helper(self.root.take(), interval_start, interval_len);
    }

    /// Helper to extend interval in tree. Returns true on found and updates it, false otherwise.
    fn extend_if_found_helper(
        root: &mut TreeNode,
        interval_end: usize,
        interval_len: usize,
    ) -> bool {
        if let Some(root) = root {
            let node_interval_end = root.interval_start + root.interval_len;
            match interval_end.cmp(&node_interval_end) {
                Ordering::Less => {
                    if Self::extend_if_found_helper(&mut root.left, interval_end, interval_len) {
                        root.update_max_len();
                        return true;
                    }
                }
                Ordering::Greater => {
                    if Self::extend_if_found_helper(&mut root.right, interval_end, interval_len) {
                        root.update_max_len();
                        return true;
                    }
                }
                Ordering::Equal => {
                    root.interval_len += interval_len;
                    root.update_max_len();
                    return true;
                }
            }
        }

        false
    }

    /// Extend interval in tree if found. Returns true on found and updates it, false otherwise.
    fn extend_if_found(&mut self, interval_end: usize, interval_len: usize) -> bool {
        Self::extend_if_found_helper(&mut self.root, interval_end, interval_len)
    }

    /// Returns a free interval to the tree.
    pub fn return_interval(&mut self, interval_start: usize, mut interval_len: usize) {
        // Merge at front of other interval if possible.
        interval_len += self.remove(interval_start + interval_len).unwrap_or(0);

        // Extend the end of other interval if possible.
        if !self.extend_if_found(interval_start, interval_len) {
            // There was no interval that could be extended. Insert a new one.
            self.insert(interval_start, interval_len);
        }
    }

    /// Recursive find length helper, returns new root and the start offset of the fit.
    fn find_len_helper(mut root: Box<Node>, wanted_len: usize) -> (TreeNode, Option<usize>) {
        let left = Node::max_len(&root.left);
        let right = Node::max_len(&root.right);

        #[derive(Eq, PartialEq)]
        enum Choice {
            Me,
            Left,
            Right,
        }

        let choices = [
            (Choice::Me, root.interval_len),
            (Choice::Left, left),
            (Choice::Right, right),
        ];

        let res = choices
            .iter()
            .filter(|(_, len)| len >= &wanted_len)
            .min_by_key(|(_, len)| len)
            .expect("caller ensured enough space was left");

        match res.0 {
            Choice::Left => {
                let (left, result) =
                    Self::find_len_helper(root.left.take().expect("left is filtered"), wanted_len);
                root.left = left;
                root.update_max_len();
                (Some(root), result)
            }
            Choice::Right => {
                let (right, result) = Self::find_len_helper(
                    root.right.take().expect("right is filtered"),
                    wanted_len,
                );
                root.right = right;
                root.update_max_len();
                (Some(root), result)
            }
            // The current node has the fit.
            Choice::Me => {
                let start = root.interval_start;
                let result = Some(start + root.interval_len - wanted_len);

                // If we are lucky, we can just shrink the interval and fix the parents.
                let root = if root.interval_len > wanted_len {
                    // We're lucky.
                    root.interval_len -= wanted_len;
                    root.update_max_len();
                    Some(root)
                } else {
                    // Unlucky, need to remove interval.
                    Self::remove_helper(root, start).0
                };

                (root, result)
            }
        }
    }

    /// Tries to find a good fit and returns the start offset of the fit if found.
    pub fn find_len(&mut self, wanted_len: usize) -> Option<usize> {
        debug_assert!(wanted_len > 0);

        // We will find a gap if the max available gap is big enough.
        if Node::max_len(&self.root) < wanted_len {
            return None;
        }

        let (root, result) = Self::find_len_helper(self.root.take().unwrap(), wanted_len);
        self.root = root;
        result
    }

    /// Remove the minimum from a node and returns the old minimum node.
    fn remove_min(mut root: Box<Node>) -> (TreeNode, Box<Node>) {
        match root.left.take() {
            Some(l) => {
                let (left, min) = Self::remove_min(l);
                root.left = left;
                (Some(Node::fixup(root)), min)
            }
            None => (root.right.take(), root),
        }
    }

    /// Recursive remove procedure, returns new root and removed length.
    fn remove_helper(mut root: Box<Node>, interval_start: usize) -> (TreeNode, Option<usize>) {
        match interval_start.cmp(&root.interval_start) {
            Ordering::Less => {
                if let Some(left) = root.left.take() {
                    let (left, result) = Self::remove_helper(left, interval_start);
                    root.left = left;
                    return (Some(Node::fixup(root)), result);
                }
            }
            Ordering::Greater => {
                if let Some(right) = root.right.take() {
                    let (right, result) = Self::remove_helper(right, interval_start);
                    root.right = right;
                    return (Some(Node::fixup(root)), result);
                }
            }
            Ordering::Equal => {
                let len = root.interval_len;

                let node = if root.left.is_none() && root.right.is_none() {
                    None
                } else if root.left.is_none() {
                    root.right.take()
                } else if root.right.is_none() {
                    root.left.take()
                } else {
                    let (remaining, mut replacement) = Self::remove_min(root.right.take().unwrap());
                    replacement.left = root.left.take();
                    replacement.right = remaining;
                    Some(Node::fixup(replacement))
                };

                return (node, Some(len));
            }
        }

        (Some(root), None)
    }

    /// Remove the interval starting with `interval_start`. Returns length if removed, false otherwise.
    pub fn remove(&mut self, interval_start: usize) -> Option<usize> {
        if let Some(root) = self.root.take() {
            let (root, result) = Self::remove_helper(root, interval_start);
            self.root = root;
            result
        } else {
            None
        }
    }
}

/// Interval tree assigner test.
#[cfg(feature = "test-interval-tree")]
pub fn test_main() {
    let mut tree = AVLIntervalTree::new();
    tree.insert(0, 100);
    assert_eq!(tree.find_len(20), Some(80));
    assert_eq!(
        tree.root,
        Some(Box::new(Node {
            interval_start: 0,
            interval_len: 80,
            left: None,
            right: None,
            max_len: 80,
            height: 1,
        }))
    );
    tree.return_interval(90, 10);
    assert_eq!(
        tree.root,
        Some(Box::new(Node {
            interval_start: 0,
            interval_len: 80,
            left: None,
            right: Some(Box::new(Node {
                interval_start: 90,
                interval_len: 10,
                left: None,
                right: None,
                max_len: 10,
                height: 1,
            })),
            max_len: 80,
            height: 2,
        }))
    );
    assert_eq!(tree.find_len(5), Some(95));
    tree.return_interval(95, 5);
    assert_eq!(
        tree.root,
        Some(Box::new(Node {
            interval_start: 0,
            interval_len: 80,
            left: None,
            right: Some(Box::new(Node {
                interval_start: 90,
                interval_len: 10,
                left: None,
                right: None,
                max_len: 10,
                height: 1,
            })),
            max_len: 80,
            height: 2,
        }))
    );
    tree.return_interval(85, 4);
    assert_eq!(
        tree.root,
        Some(Box::new(Node {
            interval_start: 85,
            interval_len: 4,
            left: Some(Box::new(Node {
                interval_start: 0,
                interval_len: 80,
                left: None,
                right: None,
                max_len: 80,
                height: 1,
            })),
            right: Some(Box::new(Node {
                interval_start: 90,
                interval_len: 10,
                left: None,
                right: None,
                max_len: 10,
                height: 1,
            })),
            max_len: 80,
            height: 2,
        }))
    );
    tree.return_interval(89, 1);
    assert_eq!(
        tree.root,
        Some(Box::new(Node {
            interval_start: 85,
            interval_len: 15,
            left: Some(Box::new(Node {
                interval_start: 0,
                interval_len: 80,
                left: None,
                right: None,
                max_len: 80,
                height: 1,
            })),
            right: None,
            max_len: 80,
            height: 2,
        }))
    );
    tree.return_interval(80, 4);
    assert_eq!(
        tree.root,
        Some(Box::new(Node {
            interval_start: 85,
            interval_len: 15,
            left: Some(Box::new(Node {
                interval_start: 0,
                interval_len: 84,
                left: None,
                right: None,
                max_len: 84,
                height: 1,
            })),
            right: None,
            max_len: 84,
            height: 2,
        }))
    );
    tree.return_interval(84, 1);
    assert_eq!(
        tree.root,
        Some(Box::new(Node {
            interval_start: 0,
            interval_len: 100,
            left: None,
            right: None,
            max_len: 100,
            height: 1,
        }))
    );
    tree.remove(0);
    assert_eq!(tree.root, None);
}

/// Interval tree assigner fragments test.
#[cfg(feature = "test-interval-tree-fragments")]
pub fn test_main() {
    let mut tree = AVLIntervalTree::new();
    tree.insert(0, 100);

    for i in 0..10 {
        let x = 100 - 10 * (i + 1);
        assert_eq!(tree.find_len(10), Some(x));
        tree.return_interval(x + 5, 5);
    }

    assert_eq!(Node::height(&tree.root), 4);

    assert_eq!(tree.find_len(10), None);
    tree.return_interval(0, 5);
    assert_eq!(tree.find_len(10), Some(0));
    tree.return_interval(0, 10);
    tree.return_interval(10, 5);
    assert_eq!(tree.find_len(20), Some(0));
    assert_eq!(tree.find_len(20), None);
    tree.return_interval(80, 5);
    tree.return_interval(90, 5);
    assert_eq!(tree.find_len(20), Some(80));
    assert_eq!(tree.find_len(20), None);
    assert_eq!(tree.find_len(5), Some(25));
}
