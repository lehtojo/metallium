use lazy_static::lazy_static;
use uart_16550::SerialPort;
use spin::Mutex;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut port = unsafe { SerialPort::new(0x3F8) };
        port.init();
        Mutex::new(port)
    };
}

pub fn write(args: ::core::fmt::Arguments) {
    use core::fmt::Write;

    SERIAL1
        .lock()
        .write_fmt(args)
        .expect("Serial write failed");
}

#[macro_export]
macro_rules! serial_write {
    ($($arg:tt)*) => {
        $crate::serial::write(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_write_line {
    () => ($crate::serial_write!("\n"));
    ($fmt:expr) => ($crate::serial_write!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_write!(concat!($fmt, "\n"), $($arg)*));
}