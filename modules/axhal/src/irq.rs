//! Interrupt management.

use axhal_cpu::trap::{IRQ, register_trap_handler};

pub use axhal_plat::irq::{handle, register, set_enable, unregister};

#[register_trap_handler(IRQ)]
fn irq_handler(vector: usize) -> bool {
    let guard = kernel_guard::NoPreempt::new();
    handle(vector);
    drop(guard); // rescheduling may occur when preemption is re-enabled.
    true
}
