use crate::serial_write;

pub fn write(args: ::core::fmt::Arguments) {
    serial_write!("{}", args);
}

#[macro_export]
macro_rules! debug_write {
    ($($arg:tt)*) => {
        $crate::debug::write(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! debug_write_line {
    () => ($crate::debug_write!("\n"));
    ($fmt:expr) => ($crate::debug_write!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::debug_write!(concat!($fmt, "\n"), $($arg)*));
}