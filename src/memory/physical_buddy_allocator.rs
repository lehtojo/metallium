use core::{alloc::Layout, mem, ptr};

use lazy_static::lazy_static;
use spin::Mutex;

use crate::{debug_write_line, memory::MiB, Region, RegionKind, Regions};

use super::{mapper, PhysicalAddress, VirtualAddress};

// Todo: Should we just make the max memory adaptive?
// pub const MAX_MEMORY: usize = 0x2000000000; // 128 GB
// pub const MAX_MEMORY: usize = 0x400000000; // 16 GB
pub const MAX_MEMORY: usize = 0x100000000; // 4 GB
pub const LAYER_COUNT: usize = 8;

pub const L0_SIZE: usize = 0x80000;
pub const L1_SIZE: usize = 0x40000;
pub const L2_SIZE: usize = 0x20000;
pub const L3_SIZE: usize = 0x10000;
pub const L4_SIZE: usize = 0x8000;
pub const L5_SIZE: usize = 0x4000;
pub const L6_SIZE: usize = 0x2000;
pub const L7_SIZE: usize = 0x1000;

pub const L0_COUNT: usize = MAX_MEMORY / L0_SIZE;
pub const L1_COUNT: usize = MAX_MEMORY / L1_SIZE;
pub const L2_COUNT: usize = MAX_MEMORY / L2_SIZE;
pub const L3_COUNT: usize = MAX_MEMORY / L3_SIZE;
pub const L4_COUNT: usize = MAX_MEMORY / L4_SIZE;
pub const L5_COUNT: usize = MAX_MEMORY / L5_SIZE;
pub const L6_COUNT: usize = MAX_MEMORY / L6_SIZE;
pub const L7_COUNT: usize = MAX_MEMORY / L7_SIZE;

pub const ALLOCATION_SIZE: usize = 
    LAYER_COUNT * mem::size_of::<Layer>() +
    (L0_COUNT + L1_COUNT + L2_COUNT + L3_COUNT + L4_COUNT + L5_COUNT + L6_COUNT + L7_COUNT) / 8;

pub struct Slab {
    next: PhysicalAddress,
    previous: PhysicalAddress
}

pub struct Layer {
    pub depth: usize,
    pub states: *mut u8, // State bitmap for all slabs in this layer
    pub size: usize,

    pub upper: *mut Layer,
    pub lower: *mut Layer,

    pub next: Option<PhysicalAddress>,
    pub last: Option<PhysicalAddress>
}

impl Layer {
    unsafe fn is_split(&self, address: PhysicalAddress) -> bool {
        assert!(address.is_aligned(self.size), "Unaligned slab address");

        // If there isn't lower layer, the slab can't be split
        if self.lower == ptr::null_mut() {
            return false;
        }

        //                                          Cases
        //
        // ... ================================== ... | ... ================================== ...
        // ... |               1                | ... | ... |               1                | ...
        // ... ================================== ... | ... ================================== ...
        // ... |       1       | ... <-- Unsplit      | ... |       1       | ... <-- Split       
        // ... ================= ...                  | ... ================= ...                 
        // ... |   0   |   0   | ...                  | ... |   1   |   0   | ...                 
        // ... ================= ...                  | ... ================= ...                 
        //                                            |
        // ... ================================== ... | ... ================================== ... 
        // ... |               1                | ... | ... |               1                | ... 
        // ... ================================== ... | ... ================================== ... 
        // ... |       1       | ... <-- Split        | ... |       1       | ... <-- Split        
        // ... ================= ...                  | ... ================= ...                  
        // ... |   0   |   1   | ...                  | ... |   1   |   1   | ...                  
        // ... ================= ...                  | ... ================= ...                  

        // If the slab on this layer is split, it must be unavailable
        let slab_index = address.value() / self.size;

        if self.is_available(slab_index) {
            return false;
        }

        // If either one of the lower slabs is unavailable, the slab on this layer must be split
        let lower = &mut *self.lower;

        let lower_slab_index = slab_index * 2;
        let lower_buddy_slab_index = lower_slab_index + 1;

        !lower.is_available(lower_slab_index) || !lower.is_available(lower_buddy_slab_index)
    }

    unsafe fn split(&mut self, address: PhysicalAddress, to: usize) -> PhysicalAddress {
        let slab = address.align(self.size);
        let slab_index = slab.value() / self.size;

        // Stop when the target depth is reached
        if self.depth == to {
            self.set_unavailable(slab_index);
            return slab;
        }

        let lower = &mut *self.lower;

        if self.is_split(slab) {
            // Because the slab is already split in this layer, continue lower
            return lower.split(address, to);
        }

        assert!(self.is_available(slab_index), "Can not split a slab that is not available");

        // Since we're splitting the slab, it must be set unavailable
        self.remove(slab);
        self.set_unavailable(slab_index);

        // Compute the addresses of the two lower layer slab
        let lower_slab = address.align(lower.size).value();
        let lower_buddy_slab = PhysicalAddress::new(lower_slab ^ lower.size);

        // Set the buddy slab available on the lower layer
        lower.add(lower_buddy_slab);

        // Since we haven't reached the target depth, continue lower
        lower.split(address, to)
    }

    unsafe fn add(&mut self, address: PhysicalAddress) {
        let slab = &mut *mapper::to_kernel_mut(address.value() as *mut Slab);
        slab.next = PhysicalAddress::null();
        slab.previous = self.last.unwrap_or(PhysicalAddress::null());

        // Connect the currently last slab to this new slab
        if let Some(last) = self.last {
            let last_slab = &mut *mapper::to_kernel_mut(last.value() as *mut Slab);
            last_slab.next = address;
        }

        // Update the next available slab if there is none
        if self.next.is_none() {
            self.next = Some(address);
        }

        // Update the last available slab
        self.last = Some(address);
    }

    unsafe fn remove(&mut self, address: PhysicalAddress) {
        let slab = &mut *mapper::to_kernel_mut(address.value() as *mut Slab);

        let (previous, next) = (slab.previous, slab.next);

        // Update the previous slab to point to the next slab
        if previous != PhysicalAddress::null() {
            let previous_slab = &mut *mapper::to_kernel_mut(previous.value() as *mut Slab);
            previous_slab.next = next;
        }

        // Update the next slab to point to the previous slab
        if next != PhysicalAddress::null() {
            let next_slab = &mut *mapper::to_kernel_mut(next.value() as *mut Slab);
            next_slab.previous = previous;
        }

        // If we're removing the currently next available slab, make the second available slab the next one
        if Some(address) == self.next {
            self.next = Some(next);
        }

        // If we're removing the currently last available slab, make the second last available slab the last one
        if Some(address) == self.last {
            self.last = Some(previous);
        }
    }

    unsafe fn try_allocate(&mut self) -> Option<PhysicalAddress> {
        let slab = self.try_take()?;
        let slab_index = slab.value() / self.size;
        self.set_unavailable(slab_index);

        Some(slab)
    }

    unsafe fn is_available(&self, slab: usize) -> bool {
        let byte = slab / 8;
        let bit = slab - byte * 8;
        let value = *self.states.add(byte);
        (value >> bit) & 1 == 0
    }

    unsafe fn set_available(&mut self, slab: usize) {
        let byte = slab / 8;
        let bit = slab - byte * 8;

        *self.states.add(byte) &= !(1 << bit);
    }

    unsafe fn set_unavailable(&mut self, slab: usize) {
        let byte = slab / 8;
        let bit = slab - byte * 8;

        *self.states.add(byte) |= 1 << bit;
    }

    unsafe fn try_take(&mut self) -> Option<PhysicalAddress> {
        let slab = self.next?;

        // Set the next slab available
        let next = (*mapper::to_kernel(slab.value() as *const Slab)).next;

        if next != PhysicalAddress::null() {
            self.next = Some(next);

            let next_slab = &mut *mapper::to_kernel_mut(next.value() as *mut Slab);
            next_slab.previous = PhysicalAddress::null();
        } else {
            self.next = None;
            self.last = None;
        }

        Some(slab)
    }

    unsafe fn owns(&self, address: PhysicalAddress) -> bool {
        !self.is_available(address.value() / self.size)
    }

    unsafe fn unsplit(&mut self, address: PhysicalAddress, add: bool) {
        assert!(address.is_aligned(self.size), "Physical address was not aligned");

        let slab_index = address.value() / self.size;

        // Verify we don't "double free"
        assert!(!self.is_available(slab_index), "Slab is already available");

        // Set the slab available
        self.set_available(slab_index);

        // Compute the address of the buddy slab.
        // If the just deallocated slab is the left slab, the address should correspond to the right slab.
        // Otherwise, it should correspond to the left slab.
        let buddy_slab = PhysicalAddress::new(address.value() ^ self.size);
        let buddy_slab_index = buddy_slab.value() / self.size;

        if self.upper != ptr::null_mut() && self.is_available(buddy_slab_index) {
            // Remove the buddy slab from the available slabs
            self.remove(buddy_slab);

            // Find out the left slab
            let left_slab = PhysicalAddress::new(address.value().min(buddy_slab.value()));

            // Deallocate the upper slab, because the both lower slabs are available and merged
            let upper = &mut *self.upper;
            upper.unsplit(left_slab, add);
        } else {
            // Since we can't merge, add this slab to the available slabs
            if add {
                self.add(address);
            }
        }
    }

    unsafe fn deallocate(&mut self, address: PhysicalAddress, add: bool) {
        self.unsplit(address, add)
    }
}

pub struct PhysicalBuddyAllocator {
    layers: *mut Layer
}

unsafe impl Send for PhysicalBuddyAllocator {}

impl PhysicalBuddyAllocator {
    pub fn new() -> PhysicalBuddyAllocator {
        Self { layers: ptr::null_mut() }
    }

    // unsafe fn get_layer(&mut self, index: usize) -> &Layer {
    //     &*self.layers.add(index)
    // }

    unsafe fn get_layer_mut(&mut self, index: usize) -> &mut Layer {
        &mut *self.layers.add(index)
    }

    unsafe fn setup_layers(&mut self) {
        let mut states = self.layers.add(LAYER_COUNT) as *mut u8;
        let mut upper = self.layers.sub(1);
        let mut lower = self.layers.add(1);
        let mut count = L0_COUNT;
        let mut size = L0_SIZE;

        for depth in 0..LAYER_COUNT {
            let layer = self.layers.add(depth);
            *layer = Layer { depth, upper, lower, states, size, next: None, last: None };

            states = states.add(count / 8); // One slab takes one bit
            count *= 2; // When going deeper, slabs are split into two
            size /= 2; // When going deeper, slabs are split into two
            upper = upper.add(1);
            lower = lower.add(1);
        }

        // Fix the first and last layer
        let first_layer = self.get_layer_mut(0);
        first_layer.upper = ptr::null_mut();

        let last_layer = self.get_layer_mut(LAYER_COUNT - 1);
        last_layer.lower = ptr::null_mut();
    }

    unsafe fn reserve_region_with_largest_slabs(&mut self, region: Region) {
        if region.size() > L0_SIZE {
            let middle = region.start + region.size() / 2;
            self.reserve_region_with_largest_slabs(Region::new(region.kind, region.start, middle));
            self.reserve_region_with_largest_slabs(Region::new(region.kind, middle, region.end));
            return;
        }

        // Find out in which L0 slabs the region starts and ends.
        // Because of the recursion above, the region here can't take more than two continous L0 slabs.
        let start = PhysicalAddress::new(region.start).align(L0_SIZE).value();
        let end = PhysicalAddress::new(region.end).align(L0_SIZE).value();

        let layer = self.get_layer_mut(0);
        layer.set_unavailable(start / L0_SIZE);
        layer.set_unavailable(end / L0_SIZE);
    }

    unsafe fn reserve(&mut self, regions: &Regions, max_available_physical_address: PhysicalAddress) {
        for index in 0..regions.length {
            let region = *regions.data.add(index);

            if region.kind != RegionKind::Available && region.end <= max_available_physical_address.value() {
                self.reserve_region_with_largest_slabs(region);
            }
        }

        // Reserve the region this allocator takes
        let region = Region::new(
            RegionKind::Reserved,
            self.layers as usize,
            self.layers.add(ALLOCATION_SIZE) as usize
        );

        debug_write_line!("Physical buddy allocator: Allocator reserves {:#X}-{:#X}", region.start, region.end);
        debug_write_line!("Physical buddy allocator: Allocator reserves total of {} MiB", region.size() / MiB);

        self.reserve_region_with_largest_slabs(region);
    }

    unsafe fn add_available_slabs(&mut self, max_available_physical_address: PhysicalAddress, start: PhysicalAddress) {
        let layer = self.get_layer_mut(0);

        // If the memory ends in the middle of the last L0 slab, we don't count that as available memory,
        // because that would require processing to split it into smaller slabs as we can't provide it as a whole.
        let slabs = max_available_physical_address.align(L0_SIZE).value() / L0_SIZE;
        let start_slab = start.value().next_multiple_of(L0_SIZE) / L0_SIZE;
        let mut total = 0;

        for slab in start_slab..slabs {
            if layer.is_available(slab) {
                layer.add(PhysicalAddress::new(slab * L0_SIZE));
                total += 1;
            }
        }

        debug_write_line!("Physical buddy allocator: Total of {} available L0 slabs", total);
        debug_write_line!("Physical buddy allocator: Total of {} MiB available memory", total * L0_SIZE / MiB);
    }

    pub fn initialize(&mut self, base: PhysicalAddress, regions: &Regions, kernel_end: PhysicalAddress) -> PhysicalAddress {
        // Note: Notice how we initialize upper layer variable below
        assert!(base.value() >= mem::size_of::<Layer>(), "Physical base address is too small");
        self.layers = base.value() as *mut Layer;

        // Zero out all our memory
        unsafe {
            ptr::write_bytes(self.layers as *mut u8, 0, ALLOCATION_SIZE);
        }

        // Find where physical memory ends, so that we know where to stop
        let max_available_physical_address: PhysicalAddress =
            regions.find_end(|region| region.kind == RegionKind::Available).into();

        unsafe {
            self.setup_layers();
            self.reserve(regions, max_available_physical_address);
            self.add_available_slabs(max_available_physical_address, kernel_end);
        }

        max_available_physical_address
    }

    fn get_layer_index_by_size(bytes: usize) -> Option<usize> {
        match bytes {
            bytes if bytes > L0_SIZE => None,
            bytes if bytes > L1_SIZE => Some(0),
            bytes if bytes > L2_SIZE => Some(1),
            bytes if bytes > L3_SIZE => Some(2),
            bytes if bytes > L4_SIZE => Some(3),
            bytes if bytes > L5_SIZE => Some(4),
            bytes if bytes > L6_SIZE => Some(5),
            bytes if bytes > L7_SIZE => Some(6),
            _ => Some(7)
        }
    }

    unsafe fn allocate_physical_region(&mut self, layout: Layout) -> Option<PhysicalAddress> {
        let bytes = layout.size();

        // Find the layer where we want to allocate the specified amount of bytes
        let optimal_layer_index = Self::get_layer_index_by_size(bytes)?;

        // Attempt allocating the memory directly from the layer
        if let Some(address) = self.get_layer_mut(optimal_layer_index).try_allocate() {
            debug_write_line!(
                "Physical buddy allocator: Allocated L{} slab for {} byte(s)", optimal_layer_index, bytes
            );
            return Some(address);
        }

        // If this point is reached, it means we could not find a suitable slab for the specified amount of bytes.
        // We should look for available memory from the upper layers.
        for layer_index in (0..optimal_layer_index).rev() {
            // Look for a upper layer with an available slab.
            // Such slab is suitable for us as upper layers have larger slabs than what we require.
            // Therefore, if we find such a slab, we must split it below.
            let layer = self.get_layer_mut(layer_index);

            if let Some(slab) = layer.try_take() {
                debug_write_line!("Physical buddy allocator: Splitting L{} slab for {} byte(s)", layer_index, bytes);
                return Some(layer.split(slab, optimal_layer_index));
            }
        }

        // If this point is reached, there is no continuous slab available that could hold
        // the specified amount of memory. However, we can still try using multiple slabs to hold
        // the memory since we are using virtual addresses.
        // In addition, the size of the allocation might be problematic to determine upon deallocation.
        None
    }

    pub fn allocate(&mut self, layout: Layout) -> *mut u8 {
        let physical_address = unsafe {
            self.allocate_physical_region(layout)
                .expect("Physical buddy allocator: Out of memory")
        };

        let virtual_address = VirtualAddress::to_kernel(physical_address);

        virtual_address.value() as *mut u8
    }

    // Todo: Do we consider the specified layout properly?
    pub fn deallocate(&mut self, address: *mut u8, _layout: Layout) {
        let physical_address: PhysicalAddress = VirtualAddress::new(address as usize).into();
        assert!(physical_address.is_aligned(L7_SIZE), "Physical address was not aligned");

        unsafe {
            for index in (0..LAYER_COUNT).rev() {
                let layer = self.get_layer_mut(index);

                if layer.owns(physical_address) {
                    layer.deallocate(physical_address, true);
                    return;
                }
            }
        }
    }
}

lazy_static! {
    pub static ref instance: Mutex<PhysicalBuddyAllocator> = {
        Mutex::new(PhysicalBuddyAllocator::new())
    };
}