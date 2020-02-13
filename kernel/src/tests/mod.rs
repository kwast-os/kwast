use core::panic::PanicInfo;

pub use buddy_test::*;
pub use heap_test::*;
pub use interval_tree_test::*;
pub use vmm_test::*;

use crate::arch::qemu;

mod buddy_test;
mod heap_test;
mod interval_tree_test;
mod vmm_test;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{:#?}", info);
    unsafe { qemu::qemu_exit(1) }
}
