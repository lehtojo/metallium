use crate::{memory::VirtualAddress, low::x64::{MSR_GS_BASE, read_msr, write_msr}};
use alloc::boxed::Box;

#[repr(packed)]
pub struct Processor {
    pub padding: u64,
    pub kernel_stack_pointer: VirtualAddress,
    pub user_stack_pointer: VirtualAddress,
    pub general_kernel_stack_pointer: VirtualAddress,
    pub gdtr_physical_address: VirtualAddress,
    pub index: u32
}

impl Processor {
    pub fn create(kernel_stack_pointer: VirtualAddress, gdtr_physical_address: VirtualAddress, index: u32) -> &'static Processor {
        let processor = Box::new(Self {
            padding: 0,
            kernel_stack_pointer,
            user_stack_pointer: VirtualAddress::null(),
            general_kernel_stack_pointer: kernel_stack_pointer,
            gdtr_physical_address,
            index
        });

        // Write the processor's address to the GS register, so that the interrupt handler can access the fields
        unsafe {
            write_msr(MSR_GS_BASE, Box::as_ptr(&processor) as u64);
        }

        Box::leak(processor)
    }

    pub fn current() -> &'static mut Processor {
        unsafe {
            let address = read_msr(MSR_GS_BASE);
            &mut *(address as *mut Processor)
        }
    }
}
