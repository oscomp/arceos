# Multi-Architecture Platform Memory Expansion Configuration Guide

This document provides a detailed explanation of how to expand system memory across multiple CPU architecture platforms by modifying configuration files, including RISC-V, AArch64, LoongArch, and x86_64 architectures.

## General Instructions

Before modifying configurations on any platform, please note the following:

- All memory size parameters must be specified in hexadecimal format.
- The system must be rebuilt after any configuration changes.
- Memory parameters must be included in the run command.
- Large memory support requires the `page-alloc-4g` feature.

1. ### RISC-V 64-bit Architecture Configuration

   - #### Configuration File Modification

     **Path:** `configs/platforms/riscv64-qemu-virt.toml`

     ```
     # Physical Memory Total Size Configuration (4GB)
     # Format: Hexadecimal; underscores are used only for readability
     phys-memory-size = 0x1_0000_0000       # uint type, represents 4GB of physical memory
     ```

   - #### Build and Run Commands

     ```
     # Build and run a RISC-V system with 4GB memory
     # Parameter explanation:
     # ARCH=riscv64               - Specifies the architecture
     # PLATFORM=riscv64-qemu-virt - Specifies the platform
     # MEM=4G                     - Allocates 4GB memory to the QEMU emulator
     # LOG=debug                  - Enables debug logging
     # AX_TESTCASE=libc           - Includes libc test cases
     # BLK=y                      - Enables block device support
     # NET=y                      - Enables network support
     # FEATURES=fp_simd,page-alloc-4g - Enables floating-point SIMD and large-page memory allocation
     # ACCEL=n                    - Disables accelerator
     
     make ARCH=riscv64 PLATFORM=riscv64-qemu-virt MEM=4G LOG=debug AX_TESTCASE=libc BLK=y NET=y FEATURES=fp_simd,page-alloc-4g ACCEL=n run
     ```

2. ### AArch64 Architecture Configuration

   - #### Configuration File Modification

     **Path:** `configs/platforms/aarch64-qemu-virt.toml`

     ```
     # Physical memory total size configuration (4GB)
     # Note: The AArch64 platform usually requires memory regions to be aligned
     phys-memory-size = 0x1_0000_0000       # uint type, represents 4GB of physical memory
     ```

   - #### Build and Run Command

     ```
     # Build and run an AArch64 system with 4GB memory
     # The parameters are similar to RISC-V, but targeted for the AArch64 architecture
     make ARCH=aarch64 PLATFORM=aarch64-qemu-virt MEM=4G LOG=debug AX_TESTCASE=libc BLK=y NET=y FEATURES=fp_simd,page-alloc-4g ACCEL=n run
     ```

3. ### LoongArch 64-bit Architecture Configuration

   - #### Configuration File Modification

      **Path:** `configs/platforms/loongarch64-qemu-virt.toml`

     ```
     # Physical memory base address (starts from 0)
     phys-memory-base = 0x0000_0000        # uint type, memory start address
     
     # Total physical memory size (4GB)
     phys-memory-size = 0x1_0000_0000      # uint type, 4GB of memory
     
     # Kernel image physical base address
     # Note: Must be consistent with the page table configuration in boot.rs
     kernel-base-paddr = 0x8020_0000       # uint type, kernel load address
     
     # Kernel image virtual base address
     # LoongArch maps the kernel into high virtual address space
     kernel-base-vaddr = "0xffff_0000_8020_0000"  # string type, kernel virtual address
     ```

   - #### Boot Page Table Modification

      **Path:** `modules/axhal/src/platform/loongarch64_qemu_virt/boot.rs`

     ```rust
     /// Initialize boot page table
     /// Note: Page table entries here must match the memory configuration
     unsafe fn init_boot_page_table() {
         unsafe {
             let l1_va = va!(&raw const BOOT_PT_L1 as usize);
             
             // First-level page table entry configuration:
             // 0x0000_0000_0000 ~ 0x0080_0000_0000, use table mapping
             BOOT_PT_L0[0] = LA64PTE::new_table(crate::mem::virt_to_phys(l1_va));
             
             // Second-level page table entry configuration:
             // 0x0000_0000..0x4000_0000, 1GB large page, device memory attributes
             BOOT_PT_L1[0] = LA64PTE::new_page(
                 pa!(0),
                 MappingFlags::READ | MappingFlags::WRITE | MappingFlags::DEVICE,
                 true,  // large page flag
             );
             
             // 0x8000_0000..0xc000_0000, 1GB large page, kernel memory attributes
             // Note: Address must match the kernel-base-paddr configuration
             BOOT_PT_L1[2] = LA64PTE::new_page(
                 pa!(0x8020_0000),  // Modify to actual kernel load address
                 MappingFlags::READ | MappingFlags::WRITE | MappingFlags::EXECUTE,
                 true,  // large page flag
             );
         }
     }
     ```

   - #### QEMU Memory Setting

      **Path:** `scripts/make/qemu.mk`

     ```
     # Default memory size set to 4GB
     override MEM := 4G
     ```

   - #### Build and Run Command

     ```
     # Build and run a LoongArch system with 4GB memory
     make ARCH=loongarch64 PLATFORM=loongarch64-qemu-virt MEM=4G LOG=debug AX_TESTCASE=libc BLK=y NET=y FEATURES=fp_simd,page-alloc-4g ACCEL=n run
     ```

4. ### x86_64 Architecture Configuration (6GB Memory)

   - #### Configuration File Modification

     **Path:** `configs/platforms/x86_64-qemu-q35.toml`

     ```
     # Total physical memory size (6GB)
     phys-memory-size = 0x1_8000_0000   # uint type, 6GB memory
     
     # MMIO region configuration, format: (base address, size)
     mmio-regions = [
         [0xb000_0000, 0x1000_0000],     # PCI config space
         [0xfe00_0000, 0xc0_0000],       # PCI device region
         [0xfec0_0000, 0x1000],          # IO APIC
         [0xfed0_0000, 0x1000],          # HPET timer
         [0xfee0_0000, 0x1000],          # Local APIC
         [0xc000000000, 0x1000000000],   # 64-bit PCI address space (new region)
     ]                                   # [(uint, uint)]
     ```

   - #### Memory Management Module Modification

     **Path:** `modules/axhal/src/mem.rs`

     ```rust
     /// Get the default free memory region
     /// Returns available memory from the end of the kernel to the start of MMIO or end of physical memory
     pub(crate) fn default_free_regions() -> impl Iterator<Item = MemRegion> {
         let start = virt_to_phys((_ekernel as usize).into()).align_up_4k(); // First 4K-aligned address after kernel
         let mmio_start = pa!(0xb0000000);                                    // MMIO start address
         let phys_end = pa!(PHYS_MEMORY_BASE + PHYS_MEMORY_SIZE).align_down_4k(); // End of physical memory
     
         let end = core::cmp::min(mmio_start, phys_end); // Use the smaller of MMIO start or physical end
     
         core::iter::once(MemRegion {
             paddr: start,
             size: end.as_usize() - start.as_usize(),
             flags: MemRegionFlags::FREE | MemRegionFlags::READ | MemRegionFlags::WRITE,
             name: "free memory",
         })
     }
     ```

     **Path:** `modules/axhal/src/platform/x86_pc/mem.rs`

     ```rust
     /// Returns platform-specific memory region configuration
     pub(crate) fn platform_regions() -> impl Iterator<Item = MemRegion> {
         core::iter::once(MemRegion {
             paddr: pa!(0x1000),       // Skip first page
             size: 0x9e000,            // Traditional low memory size
             flags: MemRegionFlags::RESERVED | MemRegionFlags::READ | MemRegionFlags::WRITE,
             name: "low memory",
         })
         .chain(crate::mem::default_free_regions())     // Default free memory region
         .chain(crate::mem::default_mmio_regions())     // Default MMIO region
         .chain(core::iter::once(MemRegion {            // Extended memory region (6GB config addition)
             paddr: pa!(0x100000000),   // Starts at 4GB
             size: 0x80000000,          // Extends to 6GB (0x180000000)
             flags: MemRegionFlags::FREE | MemRegionFlags::READ | MemRegionFlags::WRITE,
             name: "extended memory 3",
         }))
     }
     ```

   - #### Boot Page Table Modification

     **Path:** `modules/axhal/src/platform/x86_pc/multiboot.S`

     ```
     /* Lower half Page Directory Pointer Table */
     .Ltmp_pdpt_low:
         .quad 0x0000 | 0x83         # 0–1GB: PRESENT | WRITABLE | HUGE_PAGE
         .quad 0x40000000 | 0x83     # 1–2GB
         .quad 0x80000000 | 0x83     # 2–3GB
         .quad 0xc0000000 | 0x83     # 3–4GB
         .quad 0x100000000 | 0x83    # 4–5GB (new)
         .quad 0x140000000 | 0x83    # 5–6GB (new)
         .zero 8 * 506               # Zero the remaining entries
     
     /* Upper half Page Directory Pointer Table (mirrored layout) */
     .Ltmp_pdpt_high:
         .quad 0x0000 | 0x83         # 0–1GB
         .quad 0x40000000 | 0x83     # 1–2GB
         .quad 0x80000000 | 0x83     # 2–3GB
         .quad 0xc0000000 | 0x83     # 3–4GB
         .quad 0x100000000 | 0x83    # 4–5GB (new)
         .quad 0x140000000 | 0x83    # 5–6GB (new)
         .zero 8 * 506               # Zero the remaining entries
     ```

   - #### Build and Run Command

     ```
     # Build and run an x86_64 system with 6GB memory
     make ARCH=x86_64 PLATFORM=x86_64-qemu-q35 MEM=6G LOG=debug AX_TESTCASE=libc BLK=y NET=y FEATURES=fp_simd,page-alloc-4g ACCEL=n run
     ```

## Notes

- A full system rebuild is required after modifying memory configuration.
- Memory mapping differs significantly across architectures.
- Large memory configurations require support from all related components (e.g., page tables, memory allocators).
- It's recommended to increase memory size incrementally and test system stability.
- Some architectures may require additional kernel or QEMU parameters to support large memory sizes.





