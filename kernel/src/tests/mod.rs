use core::panic::PanicInfo;

pub use vmm_test::*;
pub use buddy_test::*;
pub use heap_test::*;

use crate::arch::x86_64::qemu;

mod vmm_test;
mod buddy_test;
mod heap_test;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{:#?}", info);
    unsafe { qemu::qemu_exit(1) }
}
