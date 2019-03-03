#[macro_use]
pub mod vga_text;
pub mod address;
pub mod interrupts;
pub mod paging;
pub mod port;

/// Initializes arch-specific stuff.
pub fn init() {
    interrupts::init();
}

/// Halt instruction. Waits for interrupt.
pub fn halt() {
    unsafe {
        asm!("hlt" :::: "volatile");
    }
}
