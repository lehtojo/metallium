#![feature(box_as_ptr)]

#![no_std]
#![no_main]
extern crate alloc;

use core::panic::PanicInfo;

#[derive(Clone, Copy, PartialEq)]
pub enum RegionKind {
    Unknown,
    Available,
    Reserved
}

#[derive(Clone, Copy)]
pub struct Region {
    pub kind: RegionKind,
    pub start: usize,
    pub end: usize
}

impl Region {
    pub fn new(kind: RegionKind, start: usize, end: usize) -> Region {
        Self { kind, start, end }
    }

    pub fn size(&self) -> usize {
        self.end - self.start
    }
}

pub struct Regions {
    pub data: *const Region,
    pub length: usize
}

impl Regions {
    pub fn find_end<F>(&self, filter: F) -> usize where F: Fn(&Region) -> bool {
        let mut end = 0;

        for index in 0..self.length {
            let region = unsafe { *self.data.add(index) };

            if filter(&region) {
                end = end.max(region.end);
            }
        }

        end
    }
}

pub struct GraphicsInfo {
    pub framebuffer: usize,
    pub width: u32,
    pub height: u32,
    pub stride: u32
}

pub struct BootInfo {
    pub regions: Regions,
    pub kernel_regions: Regions,
    pub graphics: GraphicsInfo,
    pub rsdp_physical_address: u64
}

pub mod debug;
pub mod interrupts;
pub mod low;
pub mod memory;

use low::{x64::serial, processor::Processor};
use memory::{mapper, physical_buddy_allocator, PhysicalAddress, VirtualAddress};

unsafe fn clear_screen(info: &BootInfo) {
    for y in 0..info.graphics.height {
        for x in 0..info.graphics.width {
            let offset = (y * info.graphics.stride + x * 4) as usize;
            let pixel = (info.graphics.framebuffer + offset) as *mut u32;
            *pixel = 0xff0000ff;
        }
    }
}

unsafe fn print_region_info(info: &BootInfo) {
    for index in 0..info.regions.length {
        let region = *info.regions.data.add(index);
        let kind = match region.kind {
            RegionKind::Available => "Available",
            _ => "Reserved"
        };

        debug_write_line!("Region: range={:#X}-{:#X}, type={}", region.start, region.end, kind);
    }
}

unsafe fn allocate_physical_memory_manager(info: &BootInfo) -> PhysicalAddress {
    // Find the first available region capable of containing the physical memory allocator
    let regions = &info.regions;

    // Find the where the kernel ends
    let kernel_end: PhysicalAddress = info.kernel_regions.find_end(|_| true).into();
    debug_write_line!("Boot: Kernel ends at {:#X}", kernel_end.value());

    for index in 0..regions.length {
        let region = regions.data.add(index).as_ref().expect("Failed to access memory region");

        if region.kind == RegionKind::Available &&
            region.size() >= physical_buddy_allocator::ALLOCATION_SIZE &&
            region.end >= kernel_end.value() {
            debug_write_line!("Boot: Placing physical buddy allocator at {:#X}", region.start);

            return physical_buddy_allocator::instance.lock().initialize(
                region.start.into(),
                regions,
                kernel_end
            );
        }
    }

    panic!("Failed to allocate the physical memory manager");
}

// Todo: We need larger stack
#[repr(align(16))]
struct KernelStack([u8; 0x2000]);
static KERNEL_STACK: KernelStack = KernelStack([0; 0x2000]);

#[no_mangle]
pub unsafe extern "C" fn _start(info_pointer: *const BootInfo) -> ! {
    debug_write_line!("Boot: Entered the kernel :^)");

    let info = &*info_pointer;
    clear_screen(&info);
    print_region_info(&info);
    let max_available_physical_address = allocate_physical_memory_manager(&info);

    // We can't rely on the paging table provided by UEFI, because
    // the table might use gigantic pages (1 GiB)
    mapper::switch_to_kernel_paging_table(max_available_physical_address);

    interrupts::initialize();
    interrupts::apic::initialize(PhysicalAddress::new(info.rsdp_physical_address as usize));

    // Todo: Allocate the stack?
    let kernel_stack = KERNEL_STACK.0.as_ptr() as usize;
    let _ = Processor::create(VirtualAddress::new(kernel_stack), VirtualAddress::null(), 0);

    debug_write_line!("Done.");

    interrupts::enable();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        debug_write_line!("KERNEL PANIC :^( - {} - {}", info.message(), location);
    } else {
        debug_write_line!("KERNEL PANIC :^( - {}", info.message());
    }

    loop {}
}

