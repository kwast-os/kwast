use crate::arch::address::{PhysAddr, VirtAddr};
use crate::arch::paging::{ActiveMapping, EntryFlags};
use crate::mm::mapper::MemoryMapper;
use crate::sync::spinlock::IrqSpinlock;
use crate::util::font::{FONT_8X16, FONT_HEIGHT, FONT_WIDTH};
use core::fmt::{self, Write};
use spin::Once;

pub struct LfbParameters {
    pub address: PhysAddr,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    /// Bits per pixel.
    pub bpp: u8,
}

struct LfbText {
    address: VirtAddr,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    pitch: u32,
}

static LFB_TEXT: Once<IrqSpinlock<LfbText>> = Once::new();

/// Initializes the LFB text output.
pub fn init(
    params: LfbParameters,
    mapping: &mut ActiveMapping,
    start: VirtAddr,
) -> Option<VirtAddr> {
    if params.bpp != 32 {
        return None;
    }

    let size = params.pitch * params.height * (params.bpp as u32);
    let flags = EntryFlags::PRESENT | EntryFlags::WRITABLE | EntryFlags::NX | EntryFlags::CACHE_WC;

    mapping
        .map_range_physical(start, params.address, size as _, flags)
        .ok()?;

    LFB_TEXT.call_once(|| {
        IrqSpinlock::new(LfbText {
            address: start,
            x: 0,
            y: 0,
            width: params.width,
            height: params.height,
            pitch: params.pitch / 4,
        })
    });

    Some((start + size as _).align_up())
}

impl LfbText {
    /// Sets a pixel.
    fn set_pixel(&mut self, x: u32, y: u32, color: u32) {
        if x < self.width && y < self.height {
            // Safety: in bounds and aligned.
            unsafe {
                *self.address.as_mut::<u32>().add((y * self.pitch + x) as _) = color;
            }
        }
    }

    /// Gets a pixel.
    fn get_pixel(&mut self, x: u32, y: u32) -> u32 {
        if x < self.width && y < self.height {
            // Safety: in bounds and aligned.
            unsafe {
                *self
                    .address
                    .as_const::<u32>()
                    .add((y * self.pitch + x) as _)
            }
        } else {
            0
        }
    }

    /// Sets a character at a position in a color.
    fn set_character(&mut self, x: u32, y: u32, color: u32, mut c: u8) {
        // Handle out of range characters as spaces.
        if c < 32 || c > 127 {
            c = 32;
        }

        let c = &FONT_8X16[c as usize - 32];

        for (yo, yp) in (y * FONT_HEIGHT..y * FONT_HEIGHT + FONT_HEIGHT).enumerate() {
            for (xo, xp) in (x * FONT_WIDTH..x * FONT_WIDTH + FONT_WIDTH).enumerate() {
                let color = if c[yo] & (1 << (7 - xo)) > 0 {
                    color
                } else {
                    0
                };
                self.set_pixel(xp, yp, color);
            }
        }
    }

    /// Goes to a new line and shifts the text up if required.
    fn new_line(&mut self) {
        self.x = 0;
        self.y += 1;

        if self.y >= self.height / FONT_HEIGHT {
            // This is very slow, it would be better to have a buffer with the characters.
            // But this is only a debug output after all...
            for y in 0..self.height - FONT_HEIGHT {
                for x in 0..self.width {
                    let color = self.get_pixel(x, y + FONT_HEIGHT);
                    self.set_pixel(x, y, color);
                }
            }
            for y in self.height - FONT_HEIGHT..self.height {
                for x in 0..self.width {
                    self.set_pixel(x, y, 0);
                }
            }

            self.y -= 1;
        }
    }

    /// Writes a single character.
    fn write_character(&mut self, c: u8) {
        match c {
            b'\n' => self.new_line(),
            b'\r' => self.x = 0,
            c => {
                self.set_character(self.x, self.y, 0xffcccccc, c);

                self.x += 1;
                if self.x >= self.width / FONT_WIDTH {
                    self.new_line();
                }
            }
        }
    }
}

impl Write for LfbText {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            self.write_character(c);
        }
        Ok(())
    }
}

/// Prints a formatted string.
pub fn _print(args: fmt::Arguments) {
    if let Some(lfb_text) = LFB_TEXT.try_get() {
        lfb_text.lock().write_fmt(args).unwrap();
    }
}
