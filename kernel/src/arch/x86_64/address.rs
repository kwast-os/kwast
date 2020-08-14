use crate::arch::x86_64::paging::PAGE_SIZE;
use core::fmt::{Debug, Error, Formatter};
use core::ops::Add;
use core::ops::{AddAssign, Sub, SubAssign};

/// A 64-bit physical address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysAddr(usize);

/// A canonical form, 64-bit virtual address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(usize);

#[allow(dead_code)]
impl PhysAddr {
    /// Creates a new physical address.
    #[inline]
    pub fn new(addr: usize) -> Self {
        // Defined limit by the architecture spec.
        debug_assert_eq!(
            addr >> 52,
            0,
            "physical address cannot be more than 52-bits: {:#x}",
            addr
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

    /// Checks if the address is 2M aligned.
    pub fn is_2m_aligned(self) -> bool {
        self.0 & 0x1ff_fff == 0
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
    pub fn align_down(self) -> Self {
        PhysAddr(align_down(self.0))
    }

    /// Aligns a memory address up.
    pub fn align_up(self) -> Self {
        PhysAddr(align_up(self.0))
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

#[allow(dead_code)]
impl VirtAddr {
    /// Creates a canonical form, virtual address.
    #[inline]
    pub fn new(addr: usize) -> Self {
        debug_assert!(
            {
                let x = addr >> 47;
                x == 0 || x == 0x1ffff
            },
            "Virtual address is not in canonical form: {:#x}",
            addr
        );
        Self(addr)
    }

    /// Creates a new virtual address from a raw pointer.
    pub fn from<T>(ptr: *mut T) -> Self {
        Self::new(ptr as usize)
    }

    /// Creates a new virtual address that points to null.
    pub const fn null() -> Self {
        Self(0)
    }

    /// Checks if the address is null.
    pub fn is_null(self) -> bool {
        self.0 == 0
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

    /// Converts the virtual address to a mutable pointer.
    #[inline]
    pub fn as_mut<T>(self) -> *mut T {
        self.0 as *mut T
    }

    /// Converts the virtual address to a const pointer.
    #[inline]
    pub fn as_const<T>(self) -> *const T {
        self.0 as *const T
    }

    /// Checks if the address is page aligned.
    pub fn is_page_aligned(self) -> bool {
        self.0 & (PAGE_SIZE - 1) == 0
    }

    /// Checks if the address is 2M aligned.
    pub fn is_2m_aligned(self) -> bool {
        self.0 & 0x1ff_fff == 0
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

    /// Aligns a memory address down.
    pub fn align_down(self) -> Self {
        VirtAddr(align_down(self.0))
    }

    /// Aligns a memory address up.
    pub fn align_up(self) -> Self {
        VirtAddr(align_up(self.0))
    }
}

/// Align the value down to page size multiple.
pub fn align_down(value: usize) -> usize {
    value & !(PAGE_SIZE - 1)
}

/// Align the value up to page size multiple.
pub fn align_up(value: usize) -> usize {
    (value + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
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

impl Sub<usize> for VirtAddr {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self::Output {
        VirtAddr::new(self.0 - rhs)
    }
}

impl AddAssign<usize> for VirtAddr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

impl SubAssign<usize> for VirtAddr {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs;
    }
}
