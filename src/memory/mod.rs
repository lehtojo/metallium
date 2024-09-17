pub mod kernel_allocator;
pub mod mapper;
pub mod physical_buddy_allocator;
pub mod physical_slab_allocator;

#[allow(non_upper_case_globals)]
pub const KiB: usize = 0x400;
#[allow(non_upper_case_globals)]
pub const MiB: usize = 0x100000;
#[allow(non_upper_case_globals)]
pub const GiB: usize = 0x40000000;
pub const PAGE_SIZE: usize = 0x1000;

pub const KERNEL_CODE_SELECTOR: u16 = 0x8;
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;
pub const USER_CODE_SELECTOR: u16 = 0x18;
pub const USER_DATA_SELECTOR: u16 = 0x20;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    pub const fn new(address: usize) -> Self {
        PhysicalAddress(address)
    }

    pub const fn null() -> Self {
        PhysicalAddress(0)
    }

    pub fn is_aligned(self, alignment: usize) -> bool {
        assert!((alignment & (alignment - 1)) == 0, "Alignment must be power of two");
        (self.0 & (alignment - 1)) == 0
    }

    pub fn align(self, alignment: usize) -> Self {
        assert!((alignment & (alignment - 1)) == 0, "Alignment must be power of two");
        Self::new(self.0 & (!(alignment - 1)))
    }

    pub fn ceil(self, alignment: usize) -> Self {
        assert!((alignment & (alignment - 1)) == 0, "Alignment must be power of two");
        Self::new((self.0 + alignment - 1) & (!(alignment - 1)))
    }

    pub const fn value(self) -> usize {
        self.0
    }
}

impl From<u64> for PhysicalAddress {
    fn from(value: u64) -> Self { PhysicalAddress::new(value as usize) }
}

impl From<usize> for PhysicalAddress {
    fn from(value: usize) -> Self { PhysicalAddress::new(value) }
}

impl From<VirtualAddress> for PhysicalAddress {
    fn from(address: VirtualAddress) -> Self {
        assert!(mapper::is_kernel_address(address.value()), "Virtual address was not a kernel address");
        PhysicalAddress::new(mapper::to_physical_address(address.value()))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    pub const fn new(address: usize) -> Self {
        VirtualAddress(address)
    }

    pub const fn null() -> Self {
        VirtualAddress(0)
    }

    pub const fn to_kernel(address: PhysicalAddress) -> VirtualAddress {
        VirtualAddress(mapper::to_kernel_address(address.value()))
    }

    pub fn is_aligned(self, alignment: usize) -> bool {
        assert!((alignment & (alignment - 1)) == 0, "Alignment must be power of two");
        (self.0 & (alignment - 1)) == 0
    }

    pub fn align(self, alignment: usize) -> Self {
        assert!((alignment & (alignment - 1)) == 0, "Alignment must be power of two");
        Self(self.0 & (!(alignment - 1)))
    }

    pub fn ceil(self, alignment: usize) -> Self {
        assert!((alignment & (alignment - 1)) == 0, "Alignment must be power of two");
        Self((self.0 + alignment - 1) & (!(alignment - 1)))
    }

    pub const fn value(self) -> usize {
        self.0
    }
}