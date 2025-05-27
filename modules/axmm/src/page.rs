use core::sync::atomic::{AtomicUsize, Ordering};

use alloc::{collections::btree_map::BTreeMap, sync::Arc};
use axalloc::GlobalPage;
use axerrno::AxResult;
use axhal::mem::virt_to_phys;
use kspin::{SpinNoIrq, SpinRaw};
use lazyinit::LazyInit;
use memory_addr::PhysAddr;

static PAGE_MANAGER: LazyInit<SpinNoIrq<PageManager>> = LazyInit::new();

pub(crate) fn init_page_manager() {
    PAGE_MANAGER.init_once(SpinNoIrq::new(PageManager::new()));
}

pub fn page_manager() -> &'static SpinNoIrq<PageManager> {
    &PAGE_MANAGER
}

/// Manages the physical pages allocated in AddrSpace,
/// typically Backend::Alloc physical page frames
pub struct PageManager {
    phys2page: BTreeMap<PhysAddr, Arc<Page>>,
}

impl PageManager {
    pub fn new() -> Self {
        Self {
            phys2page: BTreeMap::new(),
        }
    }

    /// Allocate contiguous 4K-sized pages.
    ///
    /// # Parameters
    ///
    /// - `num_pages`: The number of contiguous physical pages to allocate.
    /// - `align_pow2`: The alignment requirement expressed as a power of two. The starting address
    ///   of the allocated memory will be aligned to `2^align_pow2` bytes.
    /// # Returns
    /// -  newly allocated page, or error.
    pub fn alloc(&mut self, num_pages: usize, align_pow2: usize) -> AxResult<Arc<Page>> {
        match GlobalPage::alloc_contiguous(num_pages, align_pow2) {
            Ok(page) => {
                let page = Arc::new(Page::new(page));

                assert!(
                    self.phys2page
                        .insert(page.start_paddr(), page.clone())
                        .is_none()
                );

                Ok(page.clone())
            }
            Err(e) => Err(e),
        }
    }

    /// Decrement the reference count of the page at the given physical address.
    /// When the reference count is 0, it is reclaimed by RAII
    pub fn dealloc(&mut self, paddr: PhysAddr) {
        self.dec_page_ref(paddr);
    }

    /// Increment the reference count of the page at the given physical address.
    pub fn inc_page_ref(&self, paddr: PhysAddr) {
        if let Some(page) = self.find_page(paddr) {
            page.inc_ref();
        }
    }

    /// Decrement the reference count of the page at the given physical address.
    /// When the reference count is 0, it is reclaimed by RAII
    pub fn dec_page_ref(&mut self, paddr: PhysAddr) {
        if let Some(page) = self.find_page(paddr) {
            match page.dec_ref() {
                1 => {
                    debug!("page manager => sub ref : {:#?}. ref : 0", paddr);
                    self.phys2page.remove(&paddr);
                }
                n => trace!("page manager => sub ref : {:#?}, ref : {}", paddr, n - 1),
            }
        }
    }

    /// Find the page for the given physical address.
    pub fn find_page(&self, addr: PhysAddr) -> Option<Arc<Page>> {
        if let Some((_, value)) = self.phys2page.range(..=addr).next_back() {
            if value.contain_paddr(addr) {
                Some(value.clone())
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct Page {
    inner: SpinRaw<GlobalPage>,
    // page ref count
    ref_count: AtomicUsize,
}

impl Page {
    fn new(page: GlobalPage) -> Self {
        Self {
            inner: SpinRaw::new(page),
            ref_count: AtomicUsize::new(1),
        }
    }

    fn inc_ref(&self) -> usize {
        self.ref_count.fetch_add(1, Ordering::SeqCst)
    }

    fn dec_ref(&self) -> usize {
        self.ref_count.fetch_sub(1, Ordering::SeqCst)
    }

    // Fill physical memory with zero
    pub fn zero(&self) {
        self.inner.lock().zero();
    }

    /// Get current page reference count.
    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::SeqCst)
    }

    /// Get the starting physical address of the page.
    pub fn start_paddr(&self) -> PhysAddr {
        self.inner.lock().start_paddr(virt_to_phys)
    }

    /// Check if the physical address is on the page
    pub fn contain_paddr(&self, addr: PhysAddr) -> bool {
        let page = self.inner.lock();
        let start = page.start_paddr(virt_to_phys);

        start <= addr && addr <= start + page.size()
    }

    /// Copy data from another page to this page.
    pub fn copy_form(&self, other: Arc<Page>) {
        self.inner
            .lock()
            .as_slice_mut()
            .copy_from_slice(other.inner.lock().as_slice());
    }
}
