//! Boot module adapter interface.

use crate::arch::address::VirtAddr;

/// A boot module.
#[derive(Debug)]
pub struct BootModule {
    pub start: VirtAddr,
    pub len: usize,
}

/// Trait for providing the boot modules.
pub trait BootModuleProvider: Iterator<Item = BootModule> {}
