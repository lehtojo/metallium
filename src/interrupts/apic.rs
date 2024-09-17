use crate::{debug_write_line, memory::{mapper, PhysicalAddress}};
use core::{mem, slice};

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

            // Print the siganture (assume 4 characters)
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

pub unsafe fn initialize_unsafe(rsdp_physical_address: PhysicalAddress) {
    debug_write_line!("APIC: RSDP={:#X}", rsdp_physical_address.value());

    let rsdp = &*mapper::to_kernel(rsdp_physical_address.value() as *const RSDP20);
    let madt_pointer = rsdp.find_table("APIC").expect("Failed to find MADT") as *const MADT;
    let madt = &*madt_pointer;

    debug_write_line!("APIC: MADT={:p}", madt_pointer);
    debug_write_line!("APIC: 8259 PIC = {}", (madt.flags & 1) != 0);
}

pub fn initialize(rsdp_physical_address: PhysicalAddress) {
    unsafe {
        initialize_unsafe(rsdp_physical_address);
    }
}