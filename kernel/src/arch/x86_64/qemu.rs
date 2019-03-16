use super::port::write_port32;

/// Will make QEMU exit with status (status << 1) | 1.
#[allow(dead_code)]
pub unsafe fn qemu_exit(status: u32) -> ! {
    write_port32(0xf4, status);
    loop {}
}
