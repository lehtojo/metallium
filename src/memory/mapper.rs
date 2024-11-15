use alloc::vec::Vec;

use super::{PhysicalAddress, VirtualAddress, paging_table::PagingFlags};
use crate::{low::x64::kernel_paging_table, memory::{paging_table::{PagingEntryFlags, PagingTable}, PAGE_SIZE}};
use core::mem;

const KERNEL_ENTRY_INDEX: usize = 0x100;
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

pub const fn to_physical_address(value: usize) -> usize {
    value & !KERNEL_MAP_BASE
}

// Todo: Refactor address types to use u64
pub fn to_physical_address_u64(value: u64) -> u64 {
    value & !(KERNEL_MAP_BASE as u64)
}

pub fn map_kernel_page_unaligned(physical_address: PhysicalAddress, flags: PagingFlags) -> VirtualAddress {
    let virtual_address = VirtualAddress::to_kernel(physical_address);

    let aligned_physical_address = physical_address.align(PAGE_SIZE);
    let aligned_virtual_address = virtual_address.align(PAGE_SIZE);

    let mut paging_table = kernel_paging_table();
    paging_table.map_page(aligned_virtual_address, aligned_physical_address, flags);

    virtual_address
}

pub unsafe fn switch_to_kernel_paging_table(max_available_physical_address: PhysicalAddress) {
    const L4_SIZE: usize = 0x8000000000;
    const L3_SIZE: usize = 0x40000000;
    const L2_SIZE: usize = 0x200000;

    // Compute how many L2, L3 and L4 we need
    // Note: We'll use huge pages (2 MiB) instead of traditional pages (4 KiB)
    let l4_required_count = max_available_physical_address.next_multiple_of(L4_SIZE).value() / L4_SIZE;
    let l3_required_count = max_available_physical_address.next_multiple_of(L3_SIZE).value() / L3_SIZE;
    let l2_required_count = max_available_physical_address.next_multiple_of(L2_SIZE).value() / L2_SIZE;

    // In this kernel, we assume we can cover and access all physical memory through the
    // last top-level page entry, but if we can't do that, we should panic immediately.
    // Note:
    // One top-level page entry can cover up to 512 GiB. If that ever becomes a problem,
    // we can start using the 5-level paging mechanism, where the top-level page entry can support 256 TiB.
    assert!(l4_required_count == 1, "Top-level kernel page entry can not cover all physical memory");

    const PAGE_ENTRY_COUNT_PER_LEVEL: usize = 512;
    let l4_count = PAGE_ENTRY_COUNT_PER_LEVEL;
    let l3_count = l3_required_count.next_multiple_of(PAGE_ENTRY_COUNT_PER_LEVEL);
    let l2_count = l2_required_count.next_multiple_of(PAGE_ENTRY_COUNT_PER_LEVEL);

    // Allocate all the entries
    let mut entries = Vec::<u64>::with_capacity(l4_count + l3_count + l2_count);
    let l4_base = entries.as_mut_ptr();
    let l3_base = l4_base.add(l4_count);
    let l2_base = l3_base.add(l3_count);

    // Zero out the allocated memory
    let entries_size = entries.capacity() * mem::size_of::<u64>();
    core::ptr::write_bytes(entries.as_mut_ptr(), 0, entries_size);

    let flags = (
        PagingEntryFlags::PageSizeExtension |
        PagingEntryFlags::Writable |
        PagingEntryFlags::Present
    ).bits();

    // Identity map L4s
    for index in 0..l4_required_count {
        let address = to_physical_address_u64(l3_base.add(index * PAGE_ENTRY_COUNT_PER_LEVEL) as u64);
        *l4_base.add(index) = address | flags;
    }

    // Identity map L3s
    for index in 0..l3_required_count {
        let address = to_physical_address_u64(l2_base.add(index * PAGE_ENTRY_COUNT_PER_LEVEL) as u64);
        *l3_base.add(index) = address | flags;
    }

    // Identity map L2s
    for index in 0..l2_required_count {
        *l2_base.add(index) = (index * PAGE_SIZE) as u64 | flags;
    }

    // Map the kernel page entry. More info of this at the assertion above.
    let kernel_page_entry = l4_base.add(KERNEL_ENTRY_INDEX);
    *kernel_page_entry = to_physical_address_u64(l3_base as u64) | flags;

    // Switch to our new paging table
    let paging_table = PagingTable::new(Vec::leak(entries));
    paging_table.switch();
}