use core::panic::PanicInfo;

pub use mem_test::*;

use crate::arch::x86_64::qemu;

mod mem_test;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("{:#?}", info);
    unsafe { qemu::qemu_exit(1) }
}
