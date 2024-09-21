extern "C" {
    fn ports_read_u8(port: usize) -> u8;
    fn ports_read_u16(port: usize) -> u16;
    fn ports_read_u32(port: usize) -> u32;
    fn ports_write_u8(port: usize, value: usize);
    fn ports_write_u16(port: usize, value: usize);
    fn ports_write_u32(port: usize, value: usize);
}

pub fn read_u8(port: usize) -> u8 {
    unsafe { ports_read_u8(port) }
}

pub fn read_u16(port: usize) -> u16 {
    unsafe { ports_read_u16(port) }
}

pub fn read_u32(port: usize) -> u32 {
    unsafe { ports_read_u32(port) }
}

pub fn write_u8(port: usize, value: u8) {
    unsafe { ports_write_u8(port, value as usize) }
}

pub fn write_u16(port: usize, value: u16) {
    unsafe { ports_write_u16(port, value as usize) }
}

pub fn write_u32(port: usize, value: u32) {
    unsafe { ports_write_u32(port, value as usize) }
}