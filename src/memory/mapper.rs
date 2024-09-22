use super::{PhysicalAddress, VirtualAddress, paging_table::PagingFlags};
use crate::low::x64::kernel_paging_table;

const KERNEL_MAP_BASE: usize = 0xFFFF800000000000;

pub const fn to_kernel_address(pointer: usize) -> usize {
    KERNEL_MAP_BASE + pointer
}

pub fn to_kernel<T>(pointer: *const T) -> *const T {
    unsafe {
        pointer.byte_add(KERNEL_MAP_BASE) as *const T
    }
}

pub fn to_kernel_mut<T>(pointer: *mut T) -> *mut T {
    unsafe {
        pointer.byte_add(KERNEL_MAP_BASE) as *mut T
    }
}

pub const fn is_kernel_address(value: usize) -> bool {
    (value & KERNEL_MAP_BASE) == KERNEL_MAP_BASE
}

pub fn to_physical_address(value: usize) -> usize {
    value & !KERNEL_MAP_BASE
}

pub fn map_kernel_page(physical_address: PhysicalAddress, flags: PagingFlags) -> VirtualAddress {
    let mut paging_table = kernel_paging_table();
    let virtual_address = VirtualAddress::to_kernel(physical_address);
    paging_table.map_page(virtual_address, physical_address, flags);
    virtual_address
}
