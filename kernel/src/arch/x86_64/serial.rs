use core::fmt::{self, Error};

use lazy_static::lazy_static;
use spin::Mutex;

use crate::arch::x86_64::port::read_port8;

use super::port::write_port8;

struct SerialPort {
    /// IO Port.
    port: u16,
}

lazy_static! {
    static ref PORT: Mutex<SerialPort> = Mutex::new(SerialPort::new(0x3F8));
}

#[allow(dead_code)]
impl SerialPort {
    /// Inits and creates a serial port.
    fn new(port: u16) -> Self {
        #[cfg(feature = "integration-test")]
            Self::init(port);

        Self {
            port
        }
    }

    /// Inits the serial port.
    fn init(port: u16) {
        write_port8(port + 1, 0x00);
        write_port8(port + 3, 0x80);
        write_port8(port + 0, 0x01);
        write_port8(port + 1, 0x00);
        write_port8(port + 3, 0x03);
        write_port8(port + 2, 0xc7);
        write_port8(port + 4, 0x0b);
        write_port8(port + 1, 0x01);
    }

    /// Sends a byte.
    fn send(&mut self, byte: u8) {
        while (read_port8(self.port + 0x05) & 0x20) == 0 {}
        write_port8(self.port, byte);
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        for byte in s.bytes() {
            self.send(byte);
        }

        Ok(())
    }
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::arch::x86_64::serial::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    PORT.lock().write_fmt(args).unwrap();
}
