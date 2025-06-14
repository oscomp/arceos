use crate::backend::page_iter_wrapper::PageIterWrapper;
use axalloc::global_allocator;
use axhal::mem::{phys_to_virt, virt_to_phys};
use axhal::paging::{MappingFlags, PageSize, PageTable};
use memory_addr::{PAGE_SIZE_4K, PhysAddr, VirtAddr};

#[cfg(feature = "cow")]
use crate::frameinfo::frame_table;

use super::Backend;

/// Allocates a physical frame, with an option to zero it out.
///
/// This function allocates physical memory with the specified alignment and
/// returns the corresponding physical address. If allocation fails, it returns `None`.
///
/// # Parameters
/// - `zeroed`: If `true`, the allocated memory will be zero-initialized.
/// - `align`: Alignment requirement for the allocated memory, must be a multiple of 4KiB.
///
/// # Returns
/// - `Some(PhysAddr)`: The physical address if the allocation is successful.
/// - `None`: Returned if the memory allocation fails.
///
/// # Notes
/// - This function uses the global memory allocator to allocate memory, with the size
///   determined by the `align` parameter (in page units).
/// - If `zeroed` is `true`, the function uses `unsafe` operations to zero out the memory.
/// - The allocated memory must be accessed via its physical address, which requires
///   conversion using `virt_to_phys`.
pub fn alloc_frame(zeroed: bool, align: PageSize) -> Option<PhysAddr> {
    let page_size: usize = align.into();
    let num_pages = page_size / PAGE_SIZE_4K;
    let vaddr = VirtAddr::from(global_allocator().alloc_pages(num_pages, page_size).ok()?);
    if zeroed {
        unsafe { core::ptr::write_bytes(vaddr.as_mut_ptr(), 0, page_size) };
    }
    let paddr = virt_to_phys(vaddr);

    #[cfg(feature = "cow")]
    frame_table().inc_ref(paddr);

    Some(paddr)
}

/// Frees a physical frame of memory with the specified alignment.
///
/// This function converts the given physical address to a virtual address,
/// and then frees the corresponding memory pages using the global memory allocator.
/// The size of the memory to be freed is determined by the `align` parameter,
/// which must be a multiple of 4KiB.
///
/// If `cow` feature is enabled, this function decreases the reference count associated with the frame.
/// When the reference count reaches 1, it actually frees the frame memory.
///
/// # Parameters
/// - `frame`: The physical address of the memory to be freed.
/// - `align`: The alignment requirement for the memory, must be a multiple of 4KiB.
///
/// # Notes
/// - This function assumes that the provided `frame` was allocated using `alloc_frame`,
///   otherwise undefined behavior may occur.
/// - If the deallocation fails, the function will call `panic!`. Details about
///   the failure can be obtained from the global memory allocator’s error messages.
pub fn dealloc_frame(frame: PhysAddr, align: PageSize) {
    #[cfg(feature = "cow")]
    if frame_table().dec_ref(frame) > 1 {
        return;
    }

    let vaddr = phys_to_virt(frame);
    let page_size: usize = align.into();
    let num_pages = page_size / PAGE_SIZE_4K;
    global_allocator().dealloc_pages(vaddr.as_usize(), num_pages);
}

impl Backend {
    /// Creates a new allocation mapping backend.
    pub const fn new_alloc(populate: bool, align: PageSize) -> Self {
        Self::Alloc { populate, align }
    }

    pub(crate) fn map_alloc(
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        align: PageSize,
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
            if let Some(iter) = PageIterWrapper::new(start, start + size, align) {
                for addr in iter {
                    if let Some(frame) = alloc_frame(true, align) {
                        if let Ok(tlb) = pt.map(addr, frame, align, flags) {
                            tlb.ignore(); // TLB flush on map is unnecessary, as there are no outdated mappings.
                        } else {
                            return false;
                        }
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
        align: PageSize,
    ) -> bool {
        debug!("unmap_alloc: [{:#x}, {:#x})", start, start + size);
        if let Some(iter) = PageIterWrapper::new(start, start + size, align) {
            for addr in iter {
                if let Ok((frame, _page_size, tlb)) = pt.unmap(addr) {
                    // Deallocate the physical frame if there is a mapping in the
                    // page table.
                    tlb.flush();
                    dealloc_frame(frame, align);
                } else {
                    // Deallocation is needn't if the page is not mapped.
                }
            }
        }
        true
    }

    pub(crate) fn handle_page_fault_alloc(
        vaddr: VirtAddr,
        orig_flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        align: PageSize,
    ) -> bool {
        if populate {
            false // Populated mappings should not trigger page faults.
        } else if let Some(frame) = alloc_frame(true, align) {
            // Allocate a physical frame lazily and map it to the fault address.
            // `vaddr` does not need to be aligned. It will be automatically
            // aligned during `pt.map` regardless of the page size.
            pt.map(vaddr, frame, align, orig_flags)
                .map(|tlb| tlb.flush())
                .is_ok()
        } else {
            false
        }
    }
}
