use alloc::{sync::Arc, vec::Vec};
use axalloc::GlobalPage;
use axhal::{
    mem::virt_to_phys,
    paging::{MappingFlags, PageSize, PageTable},
};
use kspin::SpinNoIrq;
use memory_addr::{PAGE_SIZE_4K, PageIter4K, PhysAddr, VirtAddr};

use super::Backend;

pub struct FrameTracker {
    inner: SpinNoIrq<Vec<Arc<Frame>>>,
}

impl FrameTracker {
    fn new() -> Self {
        Self {
            inner: SpinNoIrq::new(Vec::new()),
        }
    }

    pub fn for_each<F>(&self, f: F)
    where
        F: FnMut(&Arc<Frame>),
    {
        self.inner.lock().iter().for_each(f);
    }

    pub fn find(&self, paddr: PhysAddr) -> Option<Arc<Frame>> {
        self.inner
            .lock()
            .iter()
            .find(|frame| frame.contains(paddr))
            .map(|frame| frame.clone())
    }

    pub fn insert(&self, frame: Arc<Frame>) {
        self.inner.lock().push(frame);
    }

    pub fn remove(&self, paddr: PhysAddr) {
        let mut vec = self.inner.lock();
        let index = vec
            .iter()
            .position(|frame| frame.contains(paddr))
            .expect("Tried to remove a frame that was not present");
        vec.remove(index);
    }
}

pub struct Frame {
    inner: SpinNoIrq<GlobalPage>,
}

impl Frame {
    fn new(page: GlobalPage) -> Self {
        Self {
            inner: SpinNoIrq::new(page),
        }
    }

    pub fn copy_from(&self, other: Arc<Frame>) {
        self.inner
            .lock()
            .as_slice_mut()
            .copy_from_slice(other.inner.lock().as_slice());
    }

    pub fn contains(&self, paddr: PhysAddr) -> bool {
        let start = self.start_paddr();
        let size = self.inner.lock().size();
        // left-closed, right-open interval
        start <= paddr && paddr < start + size
    }

    pub fn start_paddr(&self) -> PhysAddr {
        self.inner.lock().start_paddr(virt_to_phys)
    }
}

/// Allocates a physical memory frame and optionally zeroes it.
///
/// # Parameters
///
/// - `zeroed`: A boolean indicating whether the allocated frame should be zero-initialized.
///
/// # Returns
///
/// Returns an `Option<Arc<Frame>>`:
/// - `Some(Arc<Frame>)`: Allocation succeeded; the frame is wrapped in a reference-counted pointer.
/// - `None`: Allocation failed (e.g., out of memory).
pub fn alloc_frame(zeroed: bool, page_size: usize) -> Option<Arc<Frame>> {
    let page_num = page_size / PAGE_SIZE_4K;
    GlobalPage::alloc_contiguous(page_num, page_size)
        .ok()
        .map(|mut page| {
            if zeroed {
                page.zero();
            }

            Arc::new(Frame::new(page))
        })
}

impl Backend {
    /// Creates a new allocation mapping backend.
    pub fn new_alloc(populate: bool) -> Self {
        Self::Alloc {
            populate,
            tracker: Arc::new(FrameTracker::new()),
        }
    }

    pub(crate) fn map_alloc(
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        trakcer: Arc<FrameTracker>,
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
                if let Some(page) = alloc_frame(true, PAGE_SIZE_4K) {
                    if let Ok(tlb) = pt.map(addr, page.start_paddr(), PageSize::Size4K, flags) {
                        trakcer.insert(page);
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
        tracker: Arc<FrameTracker>,
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
                tracker.remove(frame);
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
        tracker: Arc<FrameTracker>,
    ) -> bool {
        if populate {
            false // Populated mappings should not trigger page faults.
        } else if let Some(page) = alloc_frame(true, PAGE_SIZE_4K) {
            // Allocate a physical frame lazily and map it to the fault address.
            // `vaddr` does not need to be aligned. It will be automatically
            // aligned during `pt.map` regardless of the page size.
            pt.map(vaddr, page.start_paddr(), PageSize::Size4K, orig_flags)
                .map(|tlb| {
                    tracker.insert(page);
                    tlb.flush()
                })
                .is_ok()
        } else {
            false
        }
    }
}
