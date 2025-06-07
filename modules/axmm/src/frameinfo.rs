//! FrameInfo
//!
//! A simple physical FrameInfo manager is provided to track and manage
//! the reference count for every 4KB memory page frame in the system.
//!
//! There is a [' FrameInfo '] struct for each physical page frame
//! that keeps track of its reference count.
//! NOTE: If the page is huge page, its [`FrameInfo`] is placed at the
//! starting physical address.
use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::vec::Vec;
use lazyinit::LazyInit;
use memory_addr::PhysAddr;

// 4 kb page
const FRAME_SHIFT: usize = 12;

pub const MAX_FRAME_NUM: usize = axconfig::plat::PHYS_MEMORY_SIZE >> FRAME_SHIFT;

static FRAME_INFO_TABLE: LazyInit<Vec<FrameInfo>> = LazyInit::new();

pub fn init_frame_info_table() {
    let _ =
        FRAME_INFO_TABLE.init_once((0..MAX_FRAME_NUM).map(|_| FrameInfo::new_empty()).collect());
}

/// Returns the `FrameInfo` structure associated with a given physical address.
///
/// # Parameters
/// - `paddr`: It must be an aligned physical address; if it's a huge page,
/// it must be the starting physical address.
///
/// # Returns
/// A reference to the `FrameInfo` associated with the given physical address.
pub fn get_frame_info(paddr: PhysAddr) -> &'static FrameInfo {
    &FRAME_INFO_TABLE[phys_to_pfn(paddr)]
}

/// Increases the reference count of the frame associated with a physical address.
///
/// # Parameters
/// - `paddr`: It must be an aligned physical address; if it's a huge page,
/// it must be the starting physical address.
pub fn add_frame_ref(paddr: PhysAddr) {
    let frame = get_frame_info(paddr);
    frame.inc_ref();
}

/// Decreases the reference count of the frame associated with a physical address.
///
/// - `paddr`: It must be an aligned physical address; if it's a huge page,
/// it must be the starting physical address.
///
/// # Returns
/// The updated reference count after decrementing.
pub fn dec_frame_ref(paddr: PhysAddr) -> usize {
    let frame = get_frame_info(paddr);
    frame.dec_ref()
}

pub struct FrameInfo {
    ref_count: AtomicUsize,
}

impl FrameInfo {
    fn new_empty() -> Self {
        Self {
            ref_count: AtomicUsize::new(0),
        }
    }

    fn inc_ref(&self) -> usize {
        self.ref_count.fetch_add(1, Ordering::SeqCst)
    }

    fn dec_ref(&self) -> usize {
        self.ref_count.fetch_sub(1, Ordering::SeqCst)
    }

    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::SeqCst)
    }
}

fn phys_to_pfn(paddr: PhysAddr) -> usize {
    assert!(paddr.as_usize() >= axconfig::plat::PHYS_MEMORY_BASE);
    (paddr.as_usize() - axconfig::plat::PHYS_MEMORY_BASE) >> FRAME_SHIFT
}
