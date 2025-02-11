use axconfig::TASK_STACK_SIZE;
use loongArch64::register::{euen, pgdh, pgdl, pwch, pwcl};

#[unsafe(link_section = ".bss.stack")]
static mut BOOT_STACK: [u8; TASK_STACK_SIZE] = [0; TASK_STACK_SIZE];

#[unsafe(link_section = ".data.boot_page_table")]
static mut BOOT_PT_L0: [u64; 512] = [0; 512];

#[unsafe(link_section = ".data.boot_page_table")]
static mut BOOT_PT_L1: [u64; 512] = [0; 512];

unsafe fn init_boot_page_table() {
    // Huge Page Mapping Flags: V | D | HUGE | P | W
    const HUGE_FLAGS: u64 = (1 << 0) | (1 << 1) | (1 << 6) | (1 << 7) | (1 << 8);
    // 0x0000_0000_0000 ~ 0x0080_0000_0000, table
    let l1_va = va!(&raw const BOOT_PT_L1 as usize);
    BOOT_PT_L0[0] = crate::mem::virt_to_phys(l1_va).as_usize() as u64;
    // 0x0000_0000..0x4000_0000, VRWX_GAD, 1G block
    BOOT_PT_L1[0] = 0x0 | HUGE_FLAGS;
    // 0x8000_0000..0xc000_0000, VRWX_GAD, 1G block
    BOOT_PT_L1[2] = 0x8000_0000 | HUGE_FLAGS;
}

unsafe fn init_mmu() {
    crate::arch::init_tlb();

    pwcl::set_pte_width(8);
    pwcl::set_ptbase(12);
    pwcl::set_ptwidth(9);
    pwcl::set_dir1_base(21);
    pwcl::set_dir1_width(9);
    pwcl::set_dir2_base(30);
    pwcl::set_dir2_width(9);
    pwch::set_dir3_base(39);
    pwch::set_dir3_width(9);

    unsafe extern "C" {
        fn handle_tlb_refill();
    }
    let paddr = crate::mem::virt_to_phys(va!(handle_tlb_refill as usize));
    crate::arch::set_tlb_refill(paddr.as_usize());

    let paddr = crate::mem::virt_to_phys(va!(&raw const BOOT_PT_L0 as usize));
    pgdh::set_base(paddr.as_usize());
    pgdl::set_base(0);
}

/// The earliest entry point for the primary CPU.
///
/// We can't use bl to jump to higher address, so we use jirl to jump to higher address.
#[naked]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
unsafe extern "C" fn _start() -> ! {
    unsafe {
        core::arch::naked_asm!("
            ori         $t0, $zero, 0x1     # CSR_DMW1_PLV0
            lu52i.d     $t0, $t0, -2048     # UC, PLV0, 0x8000 xxxx xxxx xxxx
            csrwr       $t0, 0x180          # LOONGARCH_CSR_DMWIN0
            ori         $t0, $zero, 0x11    # CSR_DMW1_MAT | CSR_DMW1_PLV0
            lu52i.d     $t0, $t0, -1792     # CA, PLV0, 0x9000 xxxx xxxx xxxx
            csrwr       $t0, 0x181          # LOONGARCH_CSR_DMWIN1

            # Setup Stack
            la.global   $sp, {boot_stack}
            li.d        $t0, {boot_stack_size}
            add.d       $sp, $sp, $t0       # setup boot stack

            # Init MMU
            bl          {init_boot_page_table}
            bl          {init_mmu}          # setup boot page table and enabel MMU
            invtlb      0x00, $r0, $r0


            # Enable PG 
            li.w		$t0, 0xb0		# PLV=0, IE=0, PG=1
            csrwr		$t0, 0x0        # LOONGARCH_CSR_CRMD
            li.w		$t0, 0x00		# PLV=0, PIE=0, PWE=0
            csrwr		$t0, 0x1        # LOONGARCH_CSR_PRMD
            li.w		$t0, 0x00		# FPE=0, SXE=0, ASXE=0, BTE=0
            csrwr		$t0, 0x2        # LOONGARCH_CSR_EUEN

            csrrd       $a0, 0x20           # cpuid
            la.global   $t0, {entry}
            jirl        $zero,$t0,0
            ",
            boot_stack_size = const TASK_STACK_SIZE,
            boot_stack = sym BOOT_STACK,
            init_boot_page_table = sym init_boot_page_table,
            init_mmu = sym init_mmu,
            entry = sym rust_tmp_main,
        )
    }
}

unsafe extern "C" {
    fn rust_main(cpu_id: usize, dtb: usize);
    #[cfg(feature = "smp")]
    fn rust_main_secondary(cpu_id: usize);
}

/// Rust temporary entry point
///
/// This function will be called after assembly boot stage.
unsafe extern "C" fn rust_tmp_main(cpu_id: usize) {
    crate::mem::clear_bss();
    axlog::ax_println!("Hello, LoongArch64!");
    crate::cpu::init_primary(cpu_id);
    rust_main(cpu_id, 0);
}

/// Initialize CPU Configuration.
fn init_cpu() {
    // Enable floating point
    euen::set_fpe(true);

    // Initialize the percpu area for this hart.
    // percpu_area_init(hart_id());

    // Initialzie Timer
    // timer::init_timer();

    // Initialize the trap and tlb fill function
    // #[cfg(feature = "trap")]
    // {
    //     trap::set_trap_vector_base();
    //     trap::tlb_init(trap::tlb_fill as _);
    // }
}

/// The entry point for the second core.
pub(crate) extern "C" fn _rust_secondary_main() {
    // Initialize CPU Configuration.
    init_cpu();

    loop {}
}
