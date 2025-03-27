//! Trap handling.

use linkme::distributed_slice as def_trap_handler;
use memory_addr::VirtAddr;
use page_table_entry::MappingFlags;

pub use linkme::distributed_slice as register_trap_handler;

use crate::arch::TrapFrame;

/// A slice of IRQ handler functions.
#[def_trap_handler]
pub static IRQ: [fn(usize) -> bool];

/// A slice of page fault handler functions.
#[def_trap_handler]
pub static PAGE_FAULT: [fn(VirtAddr, MappingFlags, bool) -> bool];

/// A slice of abitrary trap handlers.
#[def_trap_handler]
pub static ANY_TRAP: [fn(&mut TrapFrame, bool)];

/// A slice of syscall handler functions.
#[cfg(feature = "uspace")]
#[def_trap_handler]
pub static SYSCALL: [fn(&mut TrapFrame, usize) -> isize];

#[allow(unused_macros)]
macro_rules! handle_trap {
    ($trap:ident, $($args:tt)*) => {{
        let mut iter = $crate::trap::$trap.iter();
        if let Some(func) = iter.next() {
            if iter.next().is_some() {
                warn!("Multiple handlers for trap {} are not currently supported", stringify!($trap));
            }
            func($($args)*)
        } else {
            warn!("No registered handler for trap {}", stringify!($trap));
            false
        }
    }}
}

#[unsafe(no_mangle)]
pub(crate) fn handle_any_trap(tf: &mut TrapFrame, from_user: bool) {
    for handler in crate::trap::ANY_TRAP.iter() {
        handler(tf, from_user);
    }
}

/// Call the external syscall handler.
#[cfg(feature = "uspace")]
pub(crate) fn handle_syscall(tf: &mut TrapFrame, syscall_num: usize) -> isize {
    SYSCALL[0](tf, syscall_num)
}
