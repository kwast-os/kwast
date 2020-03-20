//! Boot module adapter interface.

use crate::arch::address::VirtAddr;

/// Range.
#[derive(Debug, Copy, Clone)]
pub struct Range {
    pub start: VirtAddr,
    pub len: usize,
}

/// A boot module.
#[derive(Copy, Clone, Debug)]
pub struct BootModule {
    pub range: Range,
}

/// Trait for providing the boot modules.
pub trait BootModuleProvider: Iterator<Item = BootModule> {
    /// Gives the address range where the modules are.
    fn range(&self) -> Option<Range>;
}
