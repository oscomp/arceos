use alloc::{sync::Arc, vec::Vec};
use axalloc::GlobalPage;
use axhal::{
    mem::virt_to_phys,
    paging::{MappingFlags, PageSize, PageTable},
};
use kspin::SpinRaw;
use memory_addr::{PAGE_SIZE_4K, PageIter4K, PhysAddr, VirtAddr};

use super::Backend;

pub struct PageTracker {
    inner: SpinRaw<Vec<(VirtAddr, Arc<Page>)>>,
}

impl PageTracker {
    fn new() -> Self {
        Self {
            inner: SpinRaw::new(Vec::new()),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (VirtAddr, Arc<Page>)> {
        self.inner.lock().clone().into_iter()
    }

    pub fn find(&self, paddr: PhysAddr) -> Option<Arc<Page>> {
        self.inner
            .lock()
            .iter()
            .find(|(_, page)| page.contains(paddr))
            .map(|(_, page)| page.clone())
    }

    pub fn insert(&self, vaddr: VirtAddr, page: Arc<Page>) {
        self.inner.lock().push((vaddr, page));
    }

    pub fn remove(&self, frame: PhysAddr) {
        let mut vec = self.inner.lock();
        let index = vec
            .iter()
            .position(|(_, page)| page.contains(frame))
            .expect("Tried to remove a frame that was not present");
        vec.remove(index);
    }
}

pub struct Page {
    inner: SpinRaw<GlobalPage>,
}

impl Page {
    fn new(page: GlobalPage) -> Self {
        Self {
            inner: SpinRaw::new(page),
        }
    }

    pub fn copy_from(&self, other: Arc<Page>) {
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
}

pub fn alloc_frame(zeroed: bool) -> Option<Arc<Page>> {
    GlobalPage::alloc_contiguous(1, PAGE_SIZE_4K)
        .ok()
        .map(|mut page| {
            if zeroed {
                page.zero();
            }

            Arc::new(Page::new(page))
        })
}

impl Backend {
    /// Creates a new allocation mapping backend.
    pub fn new_alloc(populate: bool) -> Self {
        Self::Alloc {
            populate,
            tracker: Arc::new(PageTracker::new()),
        }
    }

    pub(crate) fn map_alloc(
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        pt: &mut PageTable,
        populate: bool,
        trakcer: Arc<PageTracker>,
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
        tracker: Arc<PageTracker>,
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
        tracker: Arc<PageTracker>,
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
