use crate::{debug_write_line, memory::{mapper, GiB, KERNEL_CODE_SELECTOR, PAGE_SIZE}};
use core::{mem, ptr, slice};

pub mod apic;
pub mod ioapic;

extern "C" {
    fn interrupts_set_idtr(idtr: u64);
    fn interrupts_enable();
    fn interrupts_disable();
    fn interrupts_entry();

    static mut interrupts_tables: [u8; 0x3000];
}

// Todo: Should this be dynamic or is it just related to exception count on x64?
const INTERRUPT_BASE: u8 = 0x20;
const MAX_INTERRUPT_COUNT: usize = 256;
const EXCEPTION_COUNT: usize = 32;
const IDT_SIZE: usize = MAX_INTERRUPT_COUNT * mem::size_of::<IDT>();

const PRESENT_BIT: u8 = 1 << 7;

enum GateKind {
    Interrupt = 0xe,
    Trap = 0xf
}

#[repr(C)]
struct IDTR {
    size: u16,
    table: u64
}

#[repr(C)]
#[derive(Clone)]
struct IDT {
    offset_1: u16,
    selector: u16,
    interrupt_stack_table_offset: u8,
    type_attributes: u8,
    offset_2: u16,
    offset_3: u32,
    reserved: u32
}

impl IDT {
    fn empty() -> IDT {
        IDT {
            offset_1: 0,
            selector: 0,
            interrupt_stack_table_offset: 0,
            type_attributes: 0,
            offset_2: 0,
            offset_3: 0,
            reserved: 0
        }
    }
}

unsafe fn initialize_unsafe(
    idtr_address: u64,
    idt_address: u64,
    interrupt_stubs_address: u64
) {
    // Todo: We aren't using kernel addresses here and we currently crash probably to the isize cast do to inconsistency
    debug_write_line!("Interrupts: Initializing interrupt tables...");
    debug_write_line!("Interrupts: IDTR: {:#X}", idtr_address);
    debug_write_line!("Interrupts: IDT: {:#X}", idt_address);
    debug_write_line!("Interrupts: Stubs: {:#X}", interrupt_stubs_address);

    let idtr = &mut *(idtr_address as *mut IDTR);
    idtr.size = (IDT_SIZE as u16) - 1;
    idtr.table = idt_address; // IDT must be page aligned, so it can't be right after this table

    // Zero out all IDTs
    let idt = slice::from_raw_parts_mut(idt_address as *mut IDT, MAX_INTERRUPT_COUNT);
    idt.fill(IDT::empty());

    let interrupt_handler = mapper::to_kernel_address(interrupts_entry as usize) as u64;
    let mut interrupt_stub = interrupt_stubs_address as *mut u8;

    debug_write_line!("Interrupts: Interrupt handler: {:#X}", interrupt_handler);

    for interrupt_number in 0..MAX_INTERRUPT_COUNT {
        let gate = if interrupt_number < EXCEPTION_COUNT {
            GateKind::Interrupt
        } else {
            GateKind::Trap
        };

        configure_interrupt(idt, interrupt_number, gate, 0, interrupt_stub as u64);

        interrupt_stub = write_interrupt_stub(interrupt_stub, interrupt_handler, interrupt_number as u32);
    }

    debug_write_line!("Interrupts: Setting IDTR to {:#X}", idtr_address);
    interrupts_set_idtr(idtr_address);
}

pub fn initialize() {
    unsafe {
        let idtr_address = mapper::to_kernel(interrupts_tables.as_ptr()) as u64;
        let idt_address = idtr_address + (PAGE_SIZE as u64);
        let interrupt_stubs_address = idt_address + (PAGE_SIZE as u64);
        initialize_unsafe(idtr_address, idt_address, interrupt_stubs_address);
    }
}

fn configure_interrupt(
    idt: &mut [IDT],
    index: usize,
    gate: GateKind,
    privilege: u8,
    handler: u64
) {
    idt[index] = IDT {
        offset_1: handler as u16,
        selector: KERNEL_CODE_SELECTOR,
        interrupt_stack_table_offset: 1,
        type_attributes: (gate as u8) | PRESENT_BIT | ((privilege & 0b11) << 5),
        offset_2: (handler >> 16) as u16,
        offset_3: (handler >> 32) as u32,
        reserved: 0
    };
}

pub unsafe fn write_interrupt_stub(
    mut interrupt_stub: *mut u8,
    interrupt_handler: u64,
    interrupt_number: u32
) -> *mut u8 {
    if interrupt_number < EXCEPTION_COUNT as u32 {
        // push qword <interrupt>
        *interrupt_stub = 0x68;
        interrupt_stub = interrupt_stub.add(1);
        ptr::write_unaligned(interrupt_stub as *mut u32, interrupt_number);
        interrupt_stub = interrupt_stub.add(4);

        // jmp <interrupt_handler>
        let from = interrupt_stub as u64 + 5; // 5 = opcode + offset
        let offset = interrupt_handler as isize - from as isize;
        assert!(offset <= GiB as isize, "Interrupts: Too large offset to interrupt handler");

        *interrupt_stub = 0xe9;
        interrupt_stub = interrupt_stub.add(1);
        ptr::write_unaligned(interrupt_stub as *mut i32, offset as i32);
        interrupt_stub = interrupt_stub.add(4);

        return interrupt_stub.add(6); // 6 = align to 16 bytes
    }

    // push qword <interrupt>
    *interrupt_stub = 0x68;
    interrupt_stub = interrupt_stub.add(1);
    ptr::write_unaligned(interrupt_stub as *mut u32, interrupt_number);
    interrupt_stub = interrupt_stub.add(4);

    // push qword <interrupt>
    *interrupt_stub = 0x68;
    interrupt_stub = interrupt_stub.add(1);
    ptr::write_unaligned(interrupt_stub as *mut u32, interrupt_number);
    interrupt_stub = interrupt_stub.add(4);

    // jmp <interrupt_handler>
    let from = interrupt_stub as u64 + 5; // 5 = opcode + offset
    let offset = interrupt_handler as isize - from as isize;
    assert!(offset <= GiB as isize, "Interrupts: Too large offset to interrupt handler");

    *interrupt_stub = 0xe9;
    interrupt_stub = interrupt_stub.add(1);
    ptr::write_unaligned(interrupt_stub as *mut i32, offset as i32);
    interrupt_stub = interrupt_stub.add(4);

    return interrupt_stub.add(1); // 1 = align to 16 bytes
}

pub fn enable() {
    unsafe { interrupts_enable() };
}

pub fn disable() {
    unsafe { interrupts_disable() };
}

#[no_mangle]
pub fn interrupts_kernel_entry() {
    debug_write_line!("Hello Interrupt :^)");
}