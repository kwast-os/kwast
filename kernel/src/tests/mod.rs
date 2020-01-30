use core::panic::PanicInfo;

pub use vmm_test::*;
pub use buddy_test::*;

use crate::arch::x86_64::qemu;

mod vmm_test;
mod buddy_test;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{:#?}", info);
    unsafe { qemu::qemu_exit(1) }
}
