use super::{PhysicalAddress, VirtualAddress, mapper};
use crate::{debug_write_line, low::x64::write_cr3};
use alloc::{boxed::Box, vec};
use bitflags::bitflags;
use core::slice;

pub const PAGING_TABLE_ENTRY_COUNT: usize = 512;
pub const PAGE_ENTRY_PHYSICAL_ADDRESS_MASK: u64 = 0x7fffffffff000;

extern "C" {
    fn flush_tlb();
}

bitflags! {
    pub struct PagingEntryFlags: u64 {
        const Present = 1 << 0;
        const Writable = 1 << 1;
        const User = 1 << 2;
        const Cached = 1 << 4;
        const PageSizeExtension = 1 << 7;
    }
}

bitflags! {
    pub struct PagingFlags: u32 {
        const NoCache = 1 << 0;
        const NoFlush = 1 << 1;
        const User = 1 << 2;
    }
}

pub struct PagingTable<'a> {
    entries: &'a mut [u64]
}

impl<'a> PagingTable<'a> {
    pub fn new(entries: &'a mut [u64]) -> Self {
        Self { entries }
    }

    pub fn from_physical_address(physical_address: PhysicalAddress) -> PagingTable<'a> {
        let virtual_address = VirtualAddress::to_kernel(physical_address).value();
        let entries = unsafe { slice::from_raw_parts_mut(virtual_address as *mut u64, PAGING_TABLE_ENTRY_COUNT) };
        PagingTable::new(entries)
    }

    pub fn set_address(entry: &mut u64, address: u64) {
        *entry = (*entry & !PAGE_ENTRY_PHYSICAL_ADDRESS_MASK) | (address & PAGE_ENTRY_PHYSICAL_ADDRESS_MASK);
    }

    pub fn set_page_size_extension(entry: &mut u64, enabled: bool) {
        if enabled {
            *entry |= PagingEntryFlags::PageSizeExtension.bits();
        } else {
            *entry &= !PagingEntryFlags::PageSizeExtension.bits();
        }
    }

    pub fn set_user_accessability(entry: &mut u64, enabled: bool) {
        if enabled {
            *entry |= PagingEntryFlags::User.bits();
        } else {
            *entry &= !PagingEntryFlags::User.bits();
        }
    }

    pub fn set_cached(entry: &mut u64, enabled: bool) {
        if enabled {
            *entry |= PagingEntryFlags::Cached.bits();
        } else {
            *entry &= !PagingEntryFlags::Cached.bits();
        }
    }

    pub fn set_present(entry: &mut u64) {
        *entry |= PagingEntryFlags::Present.bits();
    }

    pub fn set_writable(entry: &mut u64) {
        *entry |= PagingEntryFlags::Writable.bits();
    }

    pub fn is_present(entry: u64) -> bool {
        (entry & PagingEntryFlags::Present.bits()) != 0
    }

    pub fn physical_address_from_entry(entry: u64) -> u64 {
        entry & PAGE_ENTRY_PHYSICAL_ADDRESS_MASK
    }

    pub fn map_page(&mut self, virtual_address: VirtualAddress, physical_address: PhysicalAddress, flags: PagingFlags) {
        assert!(virtual_address.is_page_aligned(), "Virtual address was not page aligned");
        assert!(physical_address.is_page_aligned(), "Physical address was not page aligned");

        debug_write_line!("Paging table: Mapping {:#X} to {:#X}", virtual_address.value(), physical_address.value());

        // Virtual address format: [L4 9 bits] [L3 9 bits] [L2 9 bits] [Offset 21 bits]
        let l2_index = (virtual_address.value() >> 21) & 0b111111111;
        let l3_index = (virtual_address.value() >> 30) & 0b111111111;
        let l4_index = (virtual_address.value() >> 39) & 0b111111111;

        let entry = &mut self.entries[l4_index];

        let l4 = if Self::is_present(*entry) {
            let physical_address = Self::physical_address_from_entry(*entry) as usize;
            let virtual_address = mapper::to_kernel_address(physical_address) as *mut u64;
            let entries = unsafe { slice::from_raw_parts_mut(virtual_address, PAGING_TABLE_ENTRY_COUNT) };
            PagingTable::new(entries)
        } else {
            let entries_memory = vec![0u64; PAGING_TABLE_ENTRY_COUNT].into_boxed_slice();
            let entries: &'static mut [u64] = Box::leak(entries_memory);
            debug_write_line!("Paging table: Created a new L4 paging table at {:p}", entries.as_ptr());

            Self::set_address(entry, entries.as_ptr() as u64);
            Self::set_writable(entry);
            Self::set_user_accessability(entry, true);
            Self::set_present(entry);

            PagingTable::new(entries)
        };

        let entry = &mut l4.entries[l3_index];

        let l3 = if Self::is_present(*entry) {
            let physical_address = Self::physical_address_from_entry(*entry) as usize;
            let virtual_address = mapper::to_kernel_address(physical_address) as *mut u64;
            let entries = unsafe { slice::from_raw_parts_mut(virtual_address, PAGING_TABLE_ENTRY_COUNT) };
            PagingTable::new(entries)
        } else {
            let entries_memory = vec![0u64; PAGING_TABLE_ENTRY_COUNT].into_boxed_slice();
            let entries: &'static mut [u64] = Box::leak(entries_memory);
            debug_write_line!("Paging table: Created a new L3 paging table at {:p}", entries.as_ptr());

            Self::set_address(entry, entries.as_ptr() as u64);
            Self::set_writable(entry);
            Self::set_user_accessability(entry, true);
            Self::set_present(entry);

            PagingTable::new(entries)
        };

        let entry = &mut l3.entries[l2_index];

        Self::set_address(entry, physical_address.value() as u64);
        Self::set_writable(entry);
        Self::set_cached(entry, !flags.contains(PagingFlags::NoCache));
        Self::set_user_accessability(entry, flags.contains(PagingFlags::User));
        Self::set_page_size_extension(entry, true);
        Self::set_present(entry);

        if !flags.contains(PagingFlags::NoFlush) {
            unsafe {
                flush_tlb();
            }
        }
    }

    pub fn switch(&self) {
        unsafe {
            let physical_address = PhysicalAddress::to_physical(VirtualAddress::new(self.entries.as_ptr() as usize));
            write_cr3(physical_address.value() as u64);
            flush_tlb(); // Todo: Verify this is needed?
        }
    }
}
