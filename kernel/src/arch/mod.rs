#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "x86_64")]
#[macro_use]
mod x86_64;

mod acpi;

pub mod asid;
pub mod cpu_data;
