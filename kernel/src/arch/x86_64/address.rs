use core::fmt::{Debug, Error, Formatter};
use core::ops::Add;

use bit_field::BitField;

use crate::arch::x86_64::paging::PAGE_SIZE;
use bitflags::_core::ops::AddAssign;

/// A 64-bit physical address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(usize);

/// A canonical form, 64-bit virtual address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(usize);

impl PhysAddr {
    /// Creates a new physical address.
    #[inline]
    pub fn new(addr: usize) -> Self {
        // Defined limit by the architecture spec.
        debug_assert_eq!(
            addr.get_bits(52..64),
            0,
            "Physical address cannot be more than 52-bits."
        );
        Self(addr)
    }

    /// Creates a new physical address that points to null.
    pub const fn null() -> Self {
        Self(0)
    }

    /// Checks if the address is null.
    pub fn is_null(self) -> bool {
        self.0 == 0
    }

    /// Checks if the address is page aligned.
    pub fn is_page_aligned(self) -> bool {
        self.0 & (PAGE_SIZE - 1) == 0
    }

    /// Converts the physical address to a usize.
    #[inline]
    pub fn as_usize(self) -> usize {
        self.0
    }

    /// Converts the physical address to a u64.
    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0 as u64
    }

    /// Aligns a memory address down.
    pub fn align_down(&self) -> Self {
        PhysAddr(self.0 & !(PAGE_SIZE - 1))
    }

    /// Aligns a memory address up.
    pub fn align_up(&self) -> Self {
        self.align_down() + PAGE_SIZE
    }
}

impl Add<usize> for PhysAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        PhysAddr::new(self.0 + rhs)
    }
}

impl AddAssign<usize> for PhysAddr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

impl Debug for PhysAddr {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "PhysAddr({:#x})", self.0)
    }
}

impl VirtAddr {
    /// Creates a canonical form, virtual address.
    #[inline]
    pub fn new(addr: usize) -> Self {
        let x = addr.get_bits(47..64);
        debug_assert!(x == 0 || x == 0x1ffff, "address is not in canonical form");
        Self(addr)
    }

    /// Converts the virtual address to a usize.
    #[inline]
    pub fn as_usize(self) -> usize {
        self.0
    }

    /// Converts the virtual address to a u64.
    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0 as u64
    }

    /// Checks if the address is page aligned.
    pub fn is_page_aligned(self) -> bool {
        self.0 & (PAGE_SIZE - 1) == 0
    }

    /// Gets the level 4 index for paging.
    pub fn p4_index(self) -> usize {
        (self.0 >> 39) & 511
    }

    /// Gets the level 3 index for paging.
    pub fn p3_index(self) -> usize {
        (self.0 >> 30) & 511
    }

    /// Gets the level 2 index for paging.
    pub fn p2_index(self) -> usize {
        (self.0 >> 21) & 511
    }

    /// Gets the level 1 index for paging.
    pub fn p1_index(self) -> usize {
        (self.0 >> 12) & 511
    }
}

impl Debug for VirtAddr {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(f, "VirtAddr({:#x})", self.0)
    }
}

impl Add<usize> for VirtAddr {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        VirtAddr::new(self.0 + rhs)
    }
}

impl AddAssign<usize> for VirtAddr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}
