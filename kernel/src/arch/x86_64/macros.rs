#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        #[cfg(not(feature = "integration-test"))]
            $crate::util::lfb_text::_print(format_args!($($arg)*));
        #[cfg(feature = "integration-test")]
            $crate::arch::serial::_print(format_args!($($arg)*));
    }
}
