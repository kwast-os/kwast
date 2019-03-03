use core::fmt::{self, Error};
use core::ptr::Unique;

use spin::Mutex;
use volatile::Volatile;

/// Represents the VGA text buffer.
type Buffer = [[Volatile<ScreenChar>; VGA_WIDTH]; VGA_HEIGHT];

/// Represents the foreground + background color of a cell.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
struct ColorCode(u8);

/// Represents a character on the screen.
#[derive(Clone, Copy)]
#[repr(C)]
struct ScreenChar {
    char: u8,
    color: ColorCode,
}

const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;

struct Writer {
    x: usize,
    color: ColorCode,
    buffer: Unique<Buffer>,
}

/// Standard VGA colors.
#[allow(dead_code)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

static WRITER: Mutex<Writer> = Mutex::new(Writer {
    x: 0,
    color: ColorCode::new(Color::Green, Color::Black),
    buffer: unsafe { Unique::new_unchecked(0xb8000 as *mut Buffer) },
});

impl ColorCode {
    /// Create a new `ColorCode` based on a foreground and a background color.
    pub const fn new(fg: Color, bg: Color) -> Self {
        ColorCode((bg as u8) << 4 | (fg as u8))
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
                let color = self.color;

                self.buffer()[y][x].write(ScreenChar { char, color });

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
        let blank = ScreenChar {
            char: b' ',
            color: self.color,
        };

        for x in 0..VGA_WIDTH {
            self.buffer()[VGA_HEIGHT - 1][x].write(blank);
        }

        // Reset
        self.x = 0;
    }

    #[allow(dead_code)]
    pub fn set_color(&mut self, color: ColorCode) {
        self.color = color;
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        for c in s.bytes() {
            self.write_char(c);
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::arch::x86_64::vga_text::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
