use crate::{debug_write_line, memory::{mapper, PhysicalAddress}};
use core::{mem, slice, ptr};

const MAX_LOCAL_APIC_COUNT: usize = 256;

#[repr(C)]
pub struct SDTHeader {
    signature: u32,
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32
}

#[repr(C)]
pub struct MADT {
    header: SDTHeader,
    local_apic_address: u32,
    flags: u32
}

#[repr(C)]
#[derive(Debug)]
pub struct MADTEntryHeader {
    kind: u8,
    length: u8
}

#[repr(C)]
#[derive(Debug)]
pub struct LocalAPICEntry {
    header: MADTEntryHeader,
    processor_id: u8,
    id: u8,
    flags: u32
}

#[repr(C)]
#[derive(Debug)]
pub struct IOAPICEntry {
    header: MADTEntryHeader,
    id: u8,
    reserved: u8,
    address: u32,
    gsi_base: u32
}

#[repr(C)]
#[derive(Debug)]
pub struct LocalAPICAddressOverrideEntry {
    header: MADTEntryHeader,
    reserved: u16,
    address: u64
}

#[repr(C)]
pub struct RSDP20 {
    signature: u64,
    checksum_1: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_address: u32,
    xsdt_address: u64,
    checksum_2: u8,
    reserved: [u8; 3]
}

impl RSDP20 {
    pub fn find_table_with_signature<T>(tables: &[T], expected_signature: u32) -> Option<*const SDTHeader>
        where T: Into<u64> + Copy
    {
        debug_write_line!("APIC: Finding table with signature {}", expected_signature);
        debug_write_line!("APIC: There are {} tables at {:p}", tables.len(), tables.as_ptr());

        for table_physical_address in tables {
            let table = mapper::to_kernel(table_physical_address.clone().into() as *const SDTHeader);
            let actual_signature = unsafe { (*table).signature };

            let signature_1 = actual_signature as u8 as char;
            let signature_2 = (actual_signature >> 8) as u8 as char;
            let signature_3 = (actual_signature >> 16) as u8 as char;
            let signature_4 = (actual_signature >> 24) as u8 as char;
            debug_write_line!(
                "APIC: Table at {:p} has signature {}{}{}{}",
                table,
                signature_1,
                signature_2,
                signature_3,
                signature_4
            );

            if actual_signature == expected_signature {
                return Some(table)
            }
        }

        None
    }

    pub fn signature_to_u32(signature: &str) -> u32 {
        let bytes = signature.as_bytes();
        assert!(bytes.len() == 4, "Signature must be exactly 4 bytes long");

        (bytes[0] as u32) |
        (bytes[1] as u32) << 8 |
        (bytes[2] as u32) << 16 |
        (bytes[3] as u32) << 24
    }

    pub fn find_table(&self, signature: &'static str) -> Option<*const SDTHeader> {
        debug_write_line!("APIC: RSDP revision: {}", self.revision);

        unsafe {
            return if self.revision == 0 {
                let rsdt_address = self.rsdt_address as u64;
                let rsdt = &*mapper::to_kernel(rsdt_address as *const SDTHeader);
                let tables_address = mapper::to_kernel(
                    (rsdt_address + mem::size_of::<SDTHeader>() as u64) as *const u32
                );
                let table_count = (rsdt.length - mem::size_of::<SDTHeader>() as u32) / 4;
                let tables = slice::from_raw_parts(tables_address, table_count as usize);

                Self::find_table_with_signature(tables, Self::signature_to_u32(&signature))
            } else if self.revision == 2 {
                let xsdt_address = self.xsdt_address as u64;
                let xsdt = &*mapper::to_kernel(xsdt_address as *const SDTHeader);
                let tables_address = mapper::to_kernel(
                    (xsdt_address + mem::size_of::<SDTHeader>() as u64) as *const u64
                );
                let table_count = (xsdt.length - mem::size_of::<SDTHeader>() as u32) / 8;
                let tables = slice::from_raw_parts(tables_address, table_count as usize);

                Self::find_table_with_signature(tables, Self::signature_to_u32(&signature))
            } else {
                panic!("APIC: Unsupported RSDP revision");
            }
        }
    }
}

struct APICInfo {
    pub local_apic_ids: [u8; MAX_LOCAL_APIC_COUNT],
    pub local_apic_count: usize,
    pub local_apic_registers: *mut u8,
    pub ioapic_registers: *mut u8
}

impl APICInfo {
    pub fn new() -> APICInfo {
        Self {
            local_apic_ids: [0; MAX_LOCAL_APIC_COUNT],
            local_apic_count: 0,
            local_apic_registers: ptr::null_mut(),
            ioapic_registers: ptr::null_mut()
        }
    }
}

impl MADT {
    unsafe fn process(&self, mut position: *const MADTEntryHeader) -> APICInfo {
        debug_write_line!("MADT: Processing entries...");

        let mut info = APICInfo::new();
        info.local_apic_registers = mapper::to_kernel_mut(self.local_apic_address as *mut u8);

        let end = position.add(self.header.length as usize - mem::size_of::<MADT>());

        while position < end {
            let entry = &*(position as *const MADTEntryHeader);

            match entry.kind {
                // Todo: Give names for the IDs
                0 => {
                    let local_apic_entry = &*(position as *const LocalAPICEntry);
                    debug_write_line!("MADT: Entry: {:?}", local_apic_entry);
                    info.local_apic_ids[info.local_apic_count] = local_apic_entry.id;
                    info.local_apic_count += 1;
                },
                1 => {
                    let ioapic_entry = &*(position as *const IOAPICEntry);
                    debug_write_line!("MADT: Entry: {:?}", ioapic_entry);

                    // Todo: Support multiple IOAPICs
                    if info.ioapic_registers == ptr::null_mut() {
                        info.ioapic_registers = ioapic_entry.address as *mut u8;
                    }
                },
                5 => {
                    let local_apic_address_override_entry = &*(position as *const LocalAPICAddressOverrideEntry);
                    debug_write_line!("MADT: Entry: {:?}", local_apic_address_override_entry);
                    info.local_apic_registers = mapper::to_kernel_mut(local_apic_address_override_entry.address as *mut u8);
                },
                _ => {
                    debug_write_line!("MADT: Unprocessed entry with id of {}", entry.kind);
                }
            }

            position = position.byte_add(entry.length as usize);
        }

        debug_write_line!("MADT: All entries processed");

        info
    }
}

pub unsafe fn initialize_unsafe(rsdp_physical_address: PhysicalAddress) {
    debug_write_line!("APIC: RSDP={:#X}", rsdp_physical_address.value());

    let rsdp = &*mapper::to_kernel(rsdp_physical_address.value() as *const RSDP20);
    let madt_pointer = rsdp.find_table("APIC").expect("Failed to find MADT") as *const MADT;
    let madt = &*madt_pointer;
    let madt_entry = madt_pointer.add(1) as *const MADTEntryHeader;
    let apic_info = madt.process(madt_entry);

    debug_write_line!("APIC: MADT={:p}", madt_pointer);
    debug_write_line!("APIC: 8259 PIC = {}", (madt.flags & 1) != 0);
}

pub fn initialize(rsdp_physical_address: PhysicalAddress) {
    unsafe {
        initialize_unsafe(rsdp_physical_address);
    }
}
