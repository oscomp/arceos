use axalloc::global_allocator;
use axhal::mem::{phys_to_virt, virt_to_phys};
use axhal::paging::{MappingFlags, PageSize, PageTable};
use memory_addr::{MemoryAddr, PAGE_SIZE_4K, PhysAddr, VirtAddr};

use crate::PAGE_SIZE_1G;

use super::Backend;

fn alloc_frame(zeroed: bool, page_size: usize) -> Option<PhysAddr> {
    let num_pages = page_size / PAGE_SIZE_4K;

    let vaddr = VirtAddr::from(
        global_allocator()
            .alloc_pages(num_pages, PAGE_SIZE_4K)
            .ok()?,
    );
    if zeroed {
        unsafe { core::ptr::write_bytes(vaddr.as_mut_ptr(), 0, PAGE_SIZE_4K) };
    }
    let paddr = virt_to_phys(vaddr);
    Some(paddr)
}

fn dealloc_frame(frame: PhysAddr, page_size: usize) {
    let num_pages = page_size / PAGE_SIZE_4K;
    let vaddr = phys_to_virt(frame);
    global_allocator().dealloc_pages(vaddr.as_usize(), num_pages);
}

fn get_page_size(page_size: usize) -> PageSize {
    match page_size {
        PAGE_SIZE_4K => PageSize::Size4K,
        PAGE_SIZE_1G => PageSize::Size1G,
        _ => PageSize::Size2M,
    }
}

impl Backend {
    /// Creates a new allocation mapping backend.
    pub const fn new_alloc(populate: bool, alignment: usize) -> Self {
        Self::Alloc {
            populate,
            alignment,
        }
    }

    pub(crate) fn map_alloc(
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        page_size: usize,
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
            let mut addr = start.align_down(page_size);
            let end = (start + size).align_up(page_size);

            while addr < end {
                if let Some(frame) = alloc_frame(true, page_size) {
                    if let Ok(tlb) = pt.map(addr, frame, get_page_size(page_size), flags) {
                        tlb.ignore(); // TLB flush on map is unnecessary, as there are no outdated mappings.
                    } else {
                        return false;
                    }
                }
                addr += page_size;
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
        page_size: usize,
    ) -> bool {
        debug!("unmap_alloc: [{:#x}, {:#x})", start, start + size);
        let mut addr = start.align_down(page_size);
        let end = (start + size).align_up(page_size);
        while addr < end {
            if let Ok((frame, _page_size, tlb)) = pt.unmap(addr) {
                // Deallocate the physical frame if there is a mapping in the
                // page table.
                tlb.flush();
                dealloc_frame(frame, page_size);
            } else {
                // Deallocation is needn't if the page is not mapped.
            }
            addr += page_size;
        }
        true
    }

    pub(crate) fn handle_page_fault_alloc(
        vaddr: VirtAddr,
        orig_flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        page_size: usize,
    ) -> bool {
        if populate {
            false // Populated mappings should not trigger page faults.
        } else if let Some(frame) = alloc_frame(true, page_size) {
            // Allocate a physical frame lazily and map it to the fault address.
            // `vaddr` does not need to be aligned. It will be automatically
            // aligned during `pt.map` regardless of the page size.
            pt.map(vaddr, frame, get_page_size(page_size), orig_flags)
                .map(|tlb| tlb.flush())
                .is_ok()
        } else {
            false
        }
    }
}
