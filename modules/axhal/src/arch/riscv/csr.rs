use crate::bin_utils::*;
use riscv::{read_csr_as, write_csr_as};

/// RISC-V Control and Status Registers (CSR) definitions

/// Supervisor Status Register
#[derive(Clone, Copy, Debug)]
pub struct CsrSstatus {
    bits: usize,
}

/// Represents the state of the RISC-V Floating-Point Unit (FPU) status (`sstatus.FS`).
///
/// The FS field (bits 13-14 of `sstatus`) tracks the FPU's state to optimize context switching.
/// It is managed cooperatively by hardware and software:
/// - **Hardware** automatically sets `Dirty` on FPU modification.
/// - **Software** (OS) reads this field to decide FPU save/restore during context switches.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum FS {
    /// FPU is disabled. Any FPU instruction execution will raise an illegal instruction exception.
    ///
    /// Used when:
    /// - The OS wants to disable FPU usage for security/performance.
    /// - A process has never requested FPU access.
    #[default]
    Off = 0,

    /// FPU is initialized but **not yet used** by the current context. No need to save/restore.
    ///
    /// Typical scenarios:
    /// - A newly created process that might use FPU later.
    /// - After explicit FPU initialization but before any FPU operations.
    Initial = 1,

    /// FPU state is **loaded but unmodified**. Requires restore on next use, but no save needed.
    ///
    /// Set by software when:
    /// - Restoring a previously saved FPU state during context switch.
    /// - The FPU registers are valid but guaranteed unchanged since last restore.
    Clean = 2,

    /// FPU state has been **modified**. Must be saved before switching to another context.
    ///
    /// Automatically set by hardware when:
    /// - Any FPU instruction (e.g., `fadd.d`, `fld`) modifies the FPU registers.
    /// Software must check this state to decide if FPU registers need saving.
    Dirty = 3,
}

read_csr_as!(CsrSstatus, 0x100);
write_csr_as!(CsrSstatus, 0x100);

impl CsrSstatus {
    const _BIT_SIE: usize = 1;
    const BIT_SPIE: usize = 5;
    const _BIT_SPP: usize = 8;
    const BIT_FS: usize = 13;
    const _BIT_XS: usize = 15;
    const BIT_SUM: usize = 18;
    const _BIT_MXR: usize = 19;
    const _BIT_UXL: usize = 32;
    const _BIT_SD: usize = usize::BITS as usize - 1;

    /// Create a new `CsrSstatus` instance with 0
    pub fn new() -> Self {
        Self { bits: 0 }
    }

    pub fn read() -> Self {
        read()
    }

    /// write the value to the CSR register SSTATUS
    pub fn save(&self) {
        write(*self);
    }

    /// SPIE: Supervisor Previous Interrupt Enable
    pub fn get_spie(&self) -> bool {
        get_bit(self.bits, Self::BIT_SPIE)
    }

    /// SPIE: Supervisor Previous Interrupt Enable
    pub fn set_spie(&mut self, value: bool) {
        if value {
            set_bit(&mut self.bits, Self::BIT_SPIE);
        } else {
            clear_bit(&mut self.bits, Self::BIT_SPIE);
        }
    }

    /// SUM: Supervisor User Memory Access
    pub fn get_sum(&self) -> bool {
        get_bit(self.bits, Self::BIT_SUM)
    }

    /// SUM: Supervisor User Memory Access
    pub fn set_sum(&mut self, value: bool) {
        if value {
            set_bit(&mut self.bits, Self::BIT_SUM);
        } else {
            clear_bit(&mut self.bits, Self::BIT_SUM);
        }
    }

    /// FS: Floating Point Status
    pub fn get_fs(&self) -> FS {
        let fs_bits = get_bits(self.bits, Self::BIT_FS, 2);
        match fs_bits {
            0 => FS::Off,
            1 => FS::Initial,
            2 => FS::Clean,
            3 => FS::Dirty,
            _ => unreachable!(),
        }
    }

    /// FS: Floating Point Status
    pub fn set_fs(&mut self, fs: FS) {
        set_bits(&mut self.bits, Self::BIT_FS, 2, fs as usize);
    }
}

impl From<usize> for CsrSstatus {
    fn from(bits: usize) -> Self {
        Self { bits }
    }
}

impl From<CsrSstatus> for usize {
    fn from(csr: CsrSstatus) -> usize {
        csr.bits
    }
}
