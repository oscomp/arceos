mod boot;

use crate::mem::phys_to_virt;
use kspin::SpinNoIrq;
use memory_addr::PhysAddr;

const UART_BASE: PhysAddr = pa!(axconfig::devices::UART_PADDR);

static UART: SpinNoIrq<LAUart> = SpinNoIrq::new(LAUart::new(phys_to_virt(UART_BASE).as_mut_ptr()));

pub struct LAUart {
    base_address: *mut u8,
}

unsafe impl Send for LAUart {}

impl LAUart {
    pub const fn new(base_address: *mut u8) -> Self {
        LAUart { base_address }
    }

    pub fn putchar(&mut self, c: u8) {
        let ptr = self.base_address;
        loop {
            unsafe {
                if ptr.add(5).read_volatile() & (1 << 5) != 0 {
                    break;
                }
            }
        }
        unsafe {
            ptr.add(0).write_volatile(c);
        }
    }

    pub fn getchar(&mut self) -> Option<u8> {
        let ptr = self.base_address;
        unsafe {
            if ptr.add(5).read_volatile() & 1 == 0 {
                // The DR bit is 0, meaning no data
                None
            } else {
                // The DR bit is 1, meaning data!
                Some(ptr.add(0).read_volatile())
            }
        }
    }
}

pub mod console {
    /// Writes bytes to the console from input u8 slice.
    pub fn write_bytes(bytes: &[u8]) {
        let mut uart = super::UART.lock();
        for &c in bytes {
            match c {
                b'\n' => {
                    uart.putchar(b'\r');
                    uart.putchar(b'\n');
                }
                c => uart.putchar(c),
            }
        }
    }

    /// Reads bytes from the console into the given mutable slice.
    /// Returns the number of bytes read.
    pub fn read_bytes(bytes: &mut [u8]) -> usize {
        let mut uart = super::UART.lock();
        for i in 0..bytes.len() {
            match uart.getchar() {
                Some(c) => bytes[i] = c,
                None => return i,
            }
        }
        bytes.len()
    }
}

pub mod misc {
    /// Shutdown the whole system, including all CPUs.
    pub fn terminate() -> ! {
        use crate::mem::phys_to_virt;
        use memory_addr::pa;
        info!("Shutting down...");
        const HALT_ADDR: *mut u8 = phys_to_virt(pa!(axconfig::devices::GED_PADDR)).as_mut_ptr();
        unsafe { HALT_ADDR.write_volatile(0x34) };
        loop {
            crate::arch::halt();
        }
    }
}

#[cfg(feature = "smp")]
pub mod mp {
    /// Starts the given secondary CPU with its boot stack.
    pub fn start_secondary_cpu(_cpu_id: usize, _stack_top: crate::mem::PhysAddr) {}
}

pub mod mem {
    /// Returns platform-specific memory regions.
    pub(crate) fn platform_regions() -> impl Iterator<Item = crate::mem::MemRegion> {
        crate::mem::default_free_regions().chain(crate::mem::default_mmio_regions())
    }
}

pub mod time {
    /// Returns the current clock time in hardware ticks.
    pub fn current_ticks() -> u64 {
        0
    }

    /// Converts hardware ticks to nanoseconds.
    pub fn ticks_to_nanos(ticks: u64) -> u64 {
        ticks
    }

    /// Converts nanoseconds to hardware ticks.
    pub fn nanos_to_ticks(nanos: u64) -> u64 {
        nanos
    }

    /// Set a one-shot timer.
    ///
    /// A timer interrupt will be triggered at the specified monotonic time deadline (in nanoseconds).
    pub fn set_oneshot_timer(_deadline_ns: u64) {}

    /// Return epoch offset in nanoseconds (wall time offset to monotonic clock start).
    pub fn epochoffset_nanos() -> u64 {
        0
    }
}

#[cfg(feature = "irq")]
pub mod irq {
    /// The maximum number of IRQs.
    pub const MAX_IRQ_COUNT: usize = 256;

    /// The timer IRQ number.
    pub const TIMER_IRQ_NUM: usize = 0;

    /// Enables or disables the given IRQ.
    pub fn set_enable(_irq_num: usize, _enabled: bool) {}

    /// Registers an IRQ handler for the given IRQ.
    pub fn register_handler(_irq_num: usize, _handler: crate::irq::IrqHandler) -> bool {
        false
    }

    /// Dispatches the IRQ.
    ///
    /// This function is called by the common interrupt handler. It looks
    /// up in the IRQ handler table and calls the corresponding handler. If
    /// necessary, it also acknowledges the interrupt controller after handling.
    pub fn dispatch_irq(_irq_num: usize) {}
}

/// Initializes the platform devices for the primary CPU.
pub fn platform_init() {}

/// Initializes the platform devices for secondary CPUs.
#[cfg(feature = "smp")]
pub fn platform_init_secondary() {}
