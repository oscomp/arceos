use alloc::{sync::Arc, vec::Vec};
use axalloc::GlobalPage;
use axhal::{
    mem::virt_to_phys,
    paging::{MappingFlags, PageSize, PageTable},
};
use kspin::SpinRaw;
use memory_addr::{PAGE_SIZE_4K, PageIter4K, PhysAddr, VirtAddr};

use crate::{PAGE_SIZE_1G, PAGE_SIZE_2M};

use super::Backend;

pub struct FrameTracker {
    inner: SpinRaw<Vec<(VirtAddr, Arc<Frame>)>>,
}

impl FrameTracker {
    fn new() -> Self {
        Self {
            inner: SpinRaw::new(Vec::new()),
        }
    }

    pub fn for_each<F>(&self, f: F)
    where
        F: FnMut(&(VirtAddr, Arc<Frame>)),
    {
        self.inner.lock().iter().for_each(f);
    }

    pub fn find(&self, addr: VirtAddr) -> Option<Arc<Frame>> {
        self.inner
            .lock()
            .iter()
            .find(|(vaddr, frame)| *vaddr <= addr && addr <= *vaddr + frame.size().into())
            .map(|(_, frame)| frame.clone())
    }

    pub fn insert(&self, vaddr: VirtAddr, frame: Arc<Frame>) {
        self.inner.lock().push((vaddr, frame));
    }

    pub fn remove(&self, paddr: PhysAddr) {
        let mut vec = self.inner.lock();
        let index = vec
            .iter()
            .position(|(_, frame)| frame.contains(paddr))
            .expect("Tried to remove a frame that was not present");
        vec.remove(index);
    }
}

pub struct Frame {
    inner: SpinRaw<GlobalPage>,
}

impl Frame {
    fn new(page: GlobalPage) -> Self {
        Self {
            inner: SpinRaw::new(page),
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

        start <= paddr && paddr <= start + size
    }

    pub fn start_paddr(&self) -> PhysAddr {
        self.inner.lock().start_paddr(virt_to_phys)
    }

    pub fn size(&self) -> PageSize {
        match self.inner.lock().size() {
            PAGE_SIZE_4K => PageSize::Size4K,
            PAGE_SIZE_2M => PageSize::Size2M,
            PAGE_SIZE_1G => PageSize::Size1G,
            _ => unreachable!(),
        }
    }
}

pub fn alloc_frame(zeroed: bool) -> Option<Arc<Frame>> {
    GlobalPage::alloc_contiguous(1, PAGE_SIZE_4K)
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
                if let Some(page) = alloc_frame(true) {
                    if let Ok(tlb) = pt.map(addr, page.start_paddr(), PageSize::Size4K, flags) {
                        trakcer.insert(addr, page);
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
        } else if let Some(page) = alloc_frame(true) {
            // Allocate a physical frame lazily and map it to the fault address.
            // `vaddr` does not need to be aligned. It will be automatically
            // aligned during `pt.map` regardless of the page size.
            pt.map(vaddr, page.start_paddr(), PageSize::Size4K, orig_flags)
                .map(|tlb| {
                    tracker.insert(vaddr, page);
                    tlb.flush()
                })
                .is_ok()
        } else {
            false
        }
    }
}
