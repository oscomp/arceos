/// Direct Memory Window Address
pub const DMW_ADDR: usize = 0x9000_0000_0000_0000;
/// Device Memory Window Address for device access
pub const DEVICE_DMW_ADDR: usize = 0x8000_0000_0000_0000;
/// Available Memory End
pub const MEMORY_END: usize = axconfig::plat::PHYS_MEMORY_BASE + axconfig::plat::PHYS_MEMORY_SIZE;
