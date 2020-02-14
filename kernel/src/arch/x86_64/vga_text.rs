use core::fmt;
use core::ptr::Unique;

use crate::sync::spinlock::Spinlock;
use volatile::Volatile;

const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;

/// Represents the VGA text buffer.
type Buffer = [[Volatile<ScreenChar>; VGA_WIDTH]; VGA_HEIGHT];

/// Represents a character on the screen.
#[derive(Clone, Copy)]
#[repr(C)]
struct ScreenChar {
    char: u8,
    color: u8,
}

struct Writer {
    x: usize,
    buffer: Unique<Buffer>,
}

static WRITER: Spinlock<Writer> = Spinlock::new(Writer {
    x: 0,
    buffer: unsafe { Unique::new_unchecked(0xb8000 as *mut Buffer) },
});

impl ScreenChar {
    /// Creates a new screen character.
    fn new(char: u8) -> Self {
        ScreenChar { char, color: 0x07 }
    }
}

impl Writer {
    fn buffer(&mut self) -> &mut Buffer {
        unsafe { self.buffer.as_mut() }
    }

    fn write_char(&mut self, c: u8) {
        match c {
            b'\n' => self.new_line(),
            char => {
                if self.x == VGA_WIDTH {
                    self.new_line();
                }

                let y = VGA_HEIGHT - 1;
                let x = self.x;

                self.buffer()[y][x].write(ScreenChar::new(char));

                self.x += 1;
            }
        }
    }

    fn new_line(&mut self) {
        // Move up
        for y in 1..VGA_HEIGHT {
            for x in 0..VGA_WIDTH {
                let c = self.buffer()[y][x].read();
                self.buffer()[y - 1][x].write(c);
            }
        }

        // Clear bottom row
        let blank = ScreenChar::new(b' ');
        for x in 0..VGA_WIDTH {
            self.buffer()[VGA_HEIGHT - 1][x].write(blank);
        }

        // Reset
        self.x = 0;
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        for c in s.bytes() {
            self.write_char(c);
        }

        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
