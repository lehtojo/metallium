use crate::memory::{PhysicalAddress, paging_table::PagingTable};

pub mod serial;

pub const MSR_GS_BASE: usize = 0xc0000101;

extern "C" {
    pub fn write_cr3(value: u64) -> u64;
    pub fn read_cr3() -> u64;

    // Note: MSR = Model Specific Register
    pub fn write_msr(id: usize, value: u64);
    pub fn read_msr(id: usize) -> u64;
}

pub fn kernel_paging_table() -> PagingTable<'static> {
    let entries_physical_address = unsafe { PhysicalAddress::new(read_cr3() as usize) };
    PagingTable::from_physical_address(entries_physical_address)
}
