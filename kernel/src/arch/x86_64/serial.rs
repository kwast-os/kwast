use core::fmt;

use lazy_static::lazy_static;

use super::port::{read_port8, write_port8};
use crate::sync::spinlock::IrqSpinlock;

struct SerialPort {
    port: u16,
}

lazy_static! {
    static ref PORT: IrqSpinlock<SerialPort> = IrqSpinlock::new(SerialPort::new(0x3F8));
}

#[allow(dead_code)]
impl SerialPort {
    /// Inits and creates a serial port.
    fn new(port: u16) -> Self {
        unsafe {
            write_port8(port + 1, 0x00);
            write_port8(port + 3, 0x80);
            write_port8(port, 0x01);
            write_port8(port + 1, 0x00);
            write_port8(port + 3, 0x03);
            write_port8(port + 2, 0xc7);
            write_port8(port + 4, 0x0b);
            write_port8(port + 1, 0x01);
        }

        Self { port }
    }

    /// Sends a byte.
    fn send(&mut self, byte: u8) {
        unsafe {
            while (read_port8(self.port + 0x05) & 0x20) == 0 {}
            write_port8(self.port, byte);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }

        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    PORT.lock().write_fmt(args).unwrap();
}
