#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "x86_64")]
#[macro_use]
pub mod x86_64;

pub mod tasking;
