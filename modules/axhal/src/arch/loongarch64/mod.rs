#[macro_use]
mod context;
mod trap;

use core::arch::asm;
use loongArch64::register::{
    crmd, ecfg, eentry, pgd, pgdh, pgdl, stlbps, tlbidx, tlbrehi, tlbrentry,
};
use memory_addr::{PhysAddr, VirtAddr};

pub use self::context::{TaskContext, TrapFrame};

#[cfg(feature = "uspace")]
pub use self::context::UspaceContext;

/// Allows the current CPU to respond to interrupts.
#[inline]
pub fn enable_irqs() {
    crmd::set_ie(true)
}

/// Makes the current CPU to ignore interrupts.
#[inline]
pub fn disable_irqs() {
    crmd::set_ie(false)
}

/// Returns whether the current CPU is allowed to respond to interrupts.
#[inline]
pub fn irqs_enabled() -> bool {
    crmd::read().ie()
}

/// Relaxes the current CPU and waits for interrupts.
///
/// It must be called with interrupts enabled, otherwise it will never return.
#[inline]
pub fn wait_for_irqs() {
    unsafe { loongArch64::asm::idle() }
}

/// Halt the current CPU.
#[inline]
pub fn halt() {
    unsafe { loongArch64::asm::idle() } // `idle` instruction in LA is different from other Archs, which should run when irqs enabled
    disable_irqs();
}

/// Reads the register that stores the current page table root.
///
/// Returns the physical address of the page table root.
#[inline]
pub fn read_page_table_root() -> PhysAddr {
    PhysAddr::from(pgd::read().base())
}

/// Writes the register to update the current page table root.
///
/// # Safety
///
/// This function is unsafe as it changes the virtual memory address space.
/// NOTE: Compiler optimize inline on release mode, kernel raise error about
/// page table. So we prohibit inline operation.
#[inline(never)]
pub fn write_page_table_root(root_paddr: PhysAddr) {
    unsafe extern "C" {
        fn trap_vector_base();
        fn handle_tlb_refill();
    }
    set_trap_vector_base(trap_vector_base as usize);

    let old_root = read_page_table_root();
    trace!("set page table root: {:#x} => {:#x}", old_root, root_paddr);
    trace!("ROOT_ADDR: 0x{:x}", root_paddr.as_usize());

    // error:
    pgdh::set_base(root_paddr.as_usize());
    pgdl::set_base(root_paddr.as_usize());
    flush_tlb(None);
    // can work:
    // unsafe {
    //     asm!(
    //         "dbar  0       ",           // sync
    //         "csrwr {root_paddr}, 0x19", // PGDL
    //         "csrwr {root_paddr}, 0x1a", // PGDH
    //         // when set pgd, MUST flush tlb. becase of old PTE in tlb.
    //         "invtlb 0x00, $r0, $r0   ", // flush tlb
    //         root_paddr = in(reg) root_paddr.as_usize(),
    //     )
    // }
    trace!("PGD_CTX  : 0x{:x}", pgd::read().base());
}

/// Flushes the TLB.
///
/// If `vaddr` is [`None`], flushes the entire TLB. Otherwise, flushes the TLB
/// entry that maps the given virtual address.
#[inline]
pub fn flush_tlb(_vaddr: Option<VirtAddr>) {
    unsafe {
        asm!("dbar 0; invtlb 0x00, $r0, $r0");
    }
}

/// Writes Exception Entry Base Address Register (`eentry`).
#[inline]
pub fn set_trap_vector_base(eentry: usize) {
    ecfg::set_vs(0);
    eentry::set_eentry(eentry);
}

#[inline]
pub fn set_tlb_refill(tlbrentry: usize) {
    tlbrentry::set_tlbrentry(tlbrentry);
}

pub const PS_4K: usize = 0x0c;
pub const PS_16K: usize = 0x0e;
pub const PS_2M: usize = 0x15;
pub const PS_1G: usize = 0x1e;

pub const PAGE_SIZE_SHIFT: usize = 12;

pub fn tlb_init(kernel_pgd_base: usize, _tlbrentry: usize) {
    // // setup PWCTL
    // unsafe {
    // asm!(
    //     "li.d     $r21,  0x4d52c",     // (9 << 15) | (21 << 10) | (9 << 5) | 12
    //     "csrwr    $r21,  0x1c",        // LOONGARCH_CSR_PWCTL0
    //     "li.d     $r21,  0x25e",       // (9 << 6)  | 30
    //     "csrwr    $r21,  0x1d",        //LOONGARCH_CSR_PWCTL1
    //     )
    // }

    pgdl::set_base(kernel_pgd_base);
    pgdh::set_base(kernel_pgd_base);
}

/// Init the TLB configuration and set tlb refill handler.
pub fn init_tlb() {
    tlbidx::set_ps(PS_4K);
    stlbps::set_ps(PS_4K);
    tlbrehi::set_ps(PS_4K);

    // // set hardware
    // pwcl::set_pte_width(8);               // 64-bits
    // pwcl::set_ptbase(12);
    // pwcl::set_ptwidth(9);
    // pwcl::set_dir1_base(21);
    // pwcl::set_dir1_width(9);
    // pwcl::set_dir2_base(30);
    // pwcl::set_dir2_width(9);

    // unsafe extern "C" {
    //     fn handle_tlb_refill();
    // }
    // let vaddr = VirtAddr::from_usize(handle_tlb_refill as usize);
    // let paddr = crate::mem::virt_to_phys(vaddr);
    // set_tlb_refill(paddr.as_usize());
}

/// Reads the thread pointer of the current CPU.
///
/// It is used to implement TLS (Thread Local Storage).
#[inline]
pub fn read_thread_pointer() -> usize {
    let tp;
    unsafe { asm!("move {}, $tp", out(reg) tp) };
    tp
}

/// Writes the thread pointer of the current CPU.
///
/// It is used to implement TLS (Thread Local Storage).
///
/// # Safety
///
/// This function is unsafe as it changes the CPU states.
#[inline]
pub unsafe fn write_thread_pointer(tp: usize) {
    unsafe { asm!("move $tp, {}", in(reg) tp) }
}

/// Initializes CPU states on the current CPU.
pub fn cpu_init() {
    unsafe extern "C" {
        fn trap_vector_base();
        fn handle_tlb_refill();
    }
    set_trap_vector_base(trap_vector_base as usize);
    crate::arch::tlb_init(0, handle_tlb_refill as usize);
}
