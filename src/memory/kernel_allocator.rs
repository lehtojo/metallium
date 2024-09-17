use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

use super::physical_buddy_allocator;

struct KernelAllocator {}

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        physical_buddy_allocator::instance.lock().allocate(layout)
    }

    unsafe fn dealloc(&self, address: *mut u8, layout: Layout) {
        physical_buddy_allocator::instance.lock().deallocate(address, layout)
    }

    unsafe fn realloc(&self, _ptr: *mut u8, _layout: Layout, _new_size: usize) -> *mut u8 {
        // Optional: Implement if resizing is needed
        ptr::null_mut()
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator {};