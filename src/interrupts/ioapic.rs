use crate::debug_write_line;

use super::INTERRUPT_BASE;

// Offset of a memory mapped register that is used to select the register to write into
const IOREGSEL: usize = 0x00;
// Offset of a memory mapped register that is used to transfer the written or read value from a register (win = window?)
const IOWIN: usize = 0x04;
// Offset to the first interrupt redirection entry (red = redirection, tbl = table)
const IOREDTBL: usize = 0x10;

const DISABLE_FLAG: u32 = 1 << 16;

pub struct IOAPIC {
    registers: *mut u32
}

impl IOAPIC {
    pub fn new(registers: *mut u32) -> Self {
        Self { registers }
    }

    fn write_register(&self, index: u32, value: u32) {
        // Select the register and write into it
        unsafe {
            *(self.registers.byte_add(IOREGSEL)) = index;
            *(self.registers.byte_add(IOWIN)) = value;
        }
    }

    fn get_redirection_entry_register(interrupt: u8) -> u32 {
        (IOREDTBL + (interrupt as usize) * 2) as u32
    }

    fn disable(&self, interrupt: u8) {
        let register = Self::get_redirection_entry_register(interrupt);
        let value = DISABLE_FLAG;
        self.write_register(register, value);
        self.write_register(register + 1, 0);
    }

    fn redirect_extended(
        &self,
        source_interrupt: u8,
        destination_interrupt: u8,
        delivery_mode: u8,
        logical_destination: bool,
        active_low: bool,
        trigger_level_mode: bool,
        masked: bool,
        cpu: u8
    ) {
        let redirection_entry_1 =
            destination_interrupt as u32 |
            ((delivery_mode as u32 & 0b111) << 8) |
            (logical_destination as u32) << 11 |
            (active_low as u32) << 13 |
            (trigger_level_mode as u32) << 15 |
            (masked as u32) << 16;

        let redirection_entry_2 = (cpu as u32) << 24;

        let register = Self::get_redirection_entry_register(source_interrupt);
        self.write_register(register, redirection_entry_1);
        self.write_register(register + 1, redirection_entry_2);
    }

    pub fn redirect(&self, interrupt: u8, cpu: u8) {
        let source_interrupt = interrupt;
        let destination_interrupt = INTERRUPT_BASE + interrupt;
        debug_write_line!(
            "IOAPIC: Redirecting interrupt {} to global interrupt {}",
            source_interrupt,
            destination_interrupt
        );

        // Disable the redirection entry before changing it
        self.disable(interrupt);
        self.redirect_extended(source_interrupt, destination_interrupt, 0, false, false, false, false, cpu);
    }
}