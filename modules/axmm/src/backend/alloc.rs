use axhal::paging::{MappingFlags, PageSize, PageTable};
use memory_addr::{PAGE_SIZE_4K, PageIter4K, PhysAddr, VirtAddr};

use crate::page::page_manager;

use super::Backend;

fn alloc_frame(zeroed: bool) -> Option<PhysAddr> {
    page_manager()
        .lock()
        .alloc(1, PAGE_SIZE_4K)
        .ok()
        .map(|page| {
            if zeroed {
                page.zero();
            }
            page.start_paddr()
        })
}

fn dealloc_frame(frame: PhysAddr) {
    page_manager().lock().dealloc(frame);
}

impl Backend {
    /// Creates a new allocation mapping backend.
    pub const fn new_alloc(populate: bool) -> Self {
        Self::Alloc { populate }
    }

    pub(crate) fn map_alloc(
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
    ) -> bool {
        debug!(
            "map_alloc: [{:#x}, {:#x}) {:?} (populate={})",
            start,
            start + size,
            flags,
            populate
        );
        if populate {
            // allocate all possible physical frames for populated mapping.
            for addr in PageIter4K::new(start, start + size).unwrap() {
                if let Some(frame) = alloc_frame(true) {
                    if let Ok(tlb) = pt.map(addr, frame, PageSize::Size4K, flags) {
                        tlb.ignore(); // TLB flush on map is unnecessary, as there are no outdated mappings.
                    } else {
                        return false;
                    }
                }
            }
        } else {
            // create mapping entries on demand later in `handle_page_fault_alloc`.
        }
        true
    }

    pub(crate) fn unmap_alloc(
        start: VirtAddr,
        size: usize,
        pt: &mut PageTable,
        _populate: bool,
    ) -> bool {
        debug!("unmap_alloc: [{:#x}, {:#x})", start, start + size);
        for addr in PageIter4K::new(start, start + size).unwrap() {
            if let Ok((frame, page_size, tlb)) = pt.unmap(addr) {
                // Deallocate the physical frame if there is a mapping in the
                // page table.
                if page_size.is_huge() {
                    return false;
                }
                tlb.flush();
                dealloc_frame(frame);
            } else {
                // Deallocation is needn't if the page is not mapped.
            }
        }
        true
    }

    pub(crate) fn handle_page_fault_alloc(
        vaddr: VirtAddr,
        orig_flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
    ) -> bool {
        if populate {
            false // Populated mappings should not trigger page faults.
        } else if let Some(frame) = alloc_frame(true) {
            // Allocate a physical frame lazily and map it to the fault address.
            // `vaddr` does not need to be aligned. It will be automatically
            // aligned during `pt.map` regardless of the page size.
            pt.map(vaddr, frame, PageSize::Size4K, orig_flags)
                .map(|tlb| tlb.flush())
                .is_ok()
        } else {
            false
        }
    }
}
