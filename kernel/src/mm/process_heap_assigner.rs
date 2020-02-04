use alloc::vec::Vec;

/// A hole.
#[derive(Debug, PartialEq, Eq)]
struct Hole {
    /// The starting point of this hole.
    start: u16,
    /// The length of this hole.
    len: u16,
}

#[derive(Debug)]
pub struct ProcessHeapAssigner {
    /// A sorted vector of holes.
    holes: Vec<Hole>,
}

impl ProcessHeapAssigner {
    /// Construct a new process heap assigner.
    pub fn new(free_amount: u16) -> Self {
        Self {
            holes: vec![Hole {
                start: 0,
                len: free_amount,
            }],
        }
    }

    /// Assign a heap.
    pub fn assign(&mut self) -> Option<u16> {
        let len = self.holes.len();

        if unlikely!(len == 0) {
            None
        } else {
            let hole = &mut self.holes[len - 1];

            let my_start = hole.start + hole.len - 1;
            if hole.len > 1 {
                hole.len -= 1;
            } else {
                self.holes.pop();
            }

            Some(my_start)
        }
    }

    /// Unassign a heap.
    pub fn unassign(&mut self, index: u16) {
        match self.holes.binary_search_by_key(&index, |h| h.start + h.len) {
            // Extends an existing run at the end.
            Ok(i) => {
                self.holes[i].len += 1;

                // Merge?
                if i + 1 < self.holes.len() && self.holes[i].start + self.holes[i].len == self.holes[i + 1].start {
                    self.holes[i].len += self.holes[i + 1].len;
                    self.holes.remove(i + 1);
                }
            }
            // Needs a new run or extends at front.
            Err(i) => {
                if i < self.holes.len() && self.holes[i].start == index + 1 {
                    // Extends at front.
                    let hole = &mut self.holes[i];
                    hole.start -= 1;
                    hole.len += 1;
                } else {
                    // New run.
                    self.holes.insert(i, Hole {
                        start: index,
                        len: 1,
                    });
                }
            }
        };
    }
}

/// Heap assigner test.
#[cfg(feature = "test-process-heap-assigner")]
pub fn test_main() {
    let mut assigner = ProcessHeapAssigner::new(6);
    assert_eq!(assigner.assign(), Some(5));
    assert_eq!(assigner.assign(), Some(4));
    assert_eq!(assigner.assign(), Some(3));
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 3,
    }]);
    assigner.unassign(4);
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 3,
    }, Hole {
        start: 4,
        len: 1,
    }]);
    assert_eq!(assigner.assign(), Some(4));
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 3,
    }]);
    assigner.unassign(4);
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 3,
    }, Hole {
        start: 4,
        len: 1,
    }]);
    assigner.unassign(5);
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 3,
    }, Hole {
        start: 4,
        len: 2,
    }]);
    assert_eq!(assigner.assign(), Some(5));
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 3,
    }, Hole {
        start: 4,
        len: 1,
    }]);
    assigner.unassign(3);
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 5,
    }]);
}

/// Heap assigner test with some fragments.
#[cfg(feature = "test-process-heap-assigner-fragments")]
pub fn test_main() {
    let mut assigner = ProcessHeapAssigner::new(10);
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 10,
    }]);
    assert_eq!(assigner.assign(), Some(9));
    assert_eq!(assigner.assign(), Some(8));
    assigner.unassign(9);
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 8,
    }, Hole {
        start: 9,
        len: 1,
    }]);
    assert_eq!(assigner.assign(), Some(9));
    assert_eq!(assigner.assign(), Some(7));
    assert_eq!(assigner.assign(), Some(6));
    assert_eq!(assigner.assign(), Some(5));
    assert_eq!(assigner.assign(), Some(4));
    assert_eq!(assigner.assign(), Some(3));
    assert_eq!(assigner.assign(), Some(2));
    assert_eq!(assigner.assign(), Some(1));
    assert_eq!(assigner.assign(), Some(0));
    assert_eq!(assigner.assign(), None);
    assigner.unassign(1);
    assigner.unassign(3);
    assigner.unassign(7);
    assigner.unassign(9);
    assert_eq!(assigner.holes, [Hole {
        start: 1,
        len: 1,
    }, Hole {
        start: 3,
        len: 1,
    }, Hole {
        start: 7,
        len: 1,
    }, Hole {
        start: 9,
        len: 1,
    }]);
    assigner.unassign(6);
    assert_eq!(assigner.holes, [Hole {
        start: 1,
        len: 1,
    }, Hole {
        start: 3,
        len: 1,
    }, Hole {
        start: 6,
        len: 2,
    }, Hole {
        start: 9,
        len: 1,
    }]);
    assigner.unassign(4);
    assert_eq!(assigner.holes, [Hole {
        start: 1,
        len: 1,
    }, Hole {
        start: 3,
        len: 2,
    }, Hole {
        start: 6,
        len: 2,
    }, Hole {
        start: 9,
        len: 1,
    }]);
    assigner.unassign(2);
    assert_eq!(assigner.holes, [Hole {
        start: 1,
        len: 4,
    }, Hole {
        start: 6,
        len: 2,
    }, Hole {
        start: 9,
        len: 1,
    }]);
    assigner.unassign(5);
    assert_eq!(assigner.holes, [Hole {
        start: 1,
        len: 7,
    }, Hole {
        start: 9,
        len: 1,
    }]);
    assigner.unassign(0);
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 8,
    }, Hole {
        start: 9,
        len: 1,
    }]);
    assigner.unassign(8);
    assert_eq!(assigner.holes, [Hole {
        start: 0,
        len: 10,
    }]);
}
