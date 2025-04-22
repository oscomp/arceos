//! [ArceOS](https://github.com/arceos-org/arceos) memory management module.

#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

use axerrno::AxResult;
use axhal::mem::phys_to_virt;
use axhal::paging::PagingHandlerImpl;
use kspin::SpinNoIrq;
use lazyinit::LazyInit;
use memory_addr::{PhysAddr, va};

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub type ArchPagingMetatData = page_table_multiarch::x86_64::X64PagingMetaData;
        pub type ArchPTE = page_table_entry::x86_64::X64PTE;
    } else if #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))] {
        pub type ArchPagingMetatData = page_table_multiarch::riscv::Sv39MetaData<memory_addr::VirtAddr>;
        pub type ArchPTE = page_table_entry::riscv::Rv64PTE;
    } else if #[cfg(target_arch = "aarch64")]{
        pub type ArchPagingMetatData = page_table_multiarch::aarch64::A64PagingMetaData;
        pub type ArchPTE = page_table_entry::aarch64::A64PTE;
    } else if #[cfg(target_arch = "loongarch64")] {
        pub type ArchPagingMetatData = page_table_multiarch::loongarch64::LA64MetaData;
        pub type ArchPTE = page_table_entry::loongarch64::LA64PTE;
    }
}

pub type AddrSpace = aspace_generic::AddrSpace<ArchPagingMetatData, ArchPTE, PagingHandlerImpl>;

static KERNEL_ASPACE: LazyInit<SpinNoIrq<AddrSpace>> = LazyInit::new();

/// Creates a new address space for kernel itself.
pub fn new_kernel_aspace() -> AxResult<AddrSpace> {
    let mut aspace = AddrSpace::new_empty(
        va!(axconfig::plat::KERNEL_ASPACE_BASE),
        axconfig::plat::KERNEL_ASPACE_SIZE,
    )?;
    for r in axhal::mem::memory_regions() {
        aspace.map_linear(phys_to_virt(r.paddr), r.paddr, r.size, r.flags.into())?;
    }
    Ok(aspace)
}

/// Returns the globally unique kernel address space.
pub fn kernel_aspace() -> &'static SpinNoIrq<AddrSpace> {
    &KERNEL_ASPACE
}

/// Returns the root physical address of the kernel page table.
pub fn kernel_page_table_root() -> PhysAddr {
    KERNEL_ASPACE.lock().page_table_root()
}

/// Initializes virtual memory management.
///
/// It mainly sets up the kernel virtual memory address space and recreate a
/// fine-grained kernel page table.
pub fn init_memory_management() {
    info!("Initialize virtual memory management...");

    let kernel_aspace = new_kernel_aspace().expect("failed to initialize kernel address space");
    debug!("kernel address space init OK: {:#x?}", kernel_aspace);
    KERNEL_ASPACE.init_once(SpinNoIrq::new(kernel_aspace));
    axhal::paging::set_kernel_page_table_root(kernel_page_table_root());
}

/// Initializes kernel paging for secondary CPUs.
pub fn init_memory_management_secondary() {
    axhal::paging::set_kernel_page_table_root(kernel_page_table_root());
}
