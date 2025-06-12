use core::fmt;

use axerrno::{AxError, AxResult, ax_err};
use axhal::mem::phys_to_virt;
use axhal::paging::{MappingFlags, PageTable, PagingError};
use memory_addr::{
    MemoryAddr, PAGE_SIZE_4K, PageIter4K, PhysAddr, VirtAddr, VirtAddrRange, is_aligned,
};
use memory_set::{MemoryArea, MemorySet};
use page_table_multiarch::PageSize;

use crate::backend::{Backend, PageIterWrapper, alloc_frame, dealloc_frame};
use crate::frameinfo::{add_frame_ref, get_frame_info};
use crate::mapping_err_to_ax_err;

/// The virtual memory address space.
pub struct AddrSpace {
    va_range: VirtAddrRange,
    areas: MemorySet<Backend>,
    pt: PageTable,
}

impl AddrSpace {
    /// Returns the address space base.
    pub const fn base(&self) -> VirtAddr {
        self.va_range.start
    }

    /// Returns the address space end.
    pub const fn end(&self) -> VirtAddr {
        self.va_range.end
    }

    /// Returns the address space size.
    pub fn size(&self) -> usize {
        self.va_range.size()
    }

    /// Returns the reference to the inner page table.
    pub const fn page_table(&self) -> &PageTable {
        &self.pt
    }

    /// Returns the root physical address of the inner page table.
    pub const fn page_table_root(&self) -> PhysAddr {
        self.pt.root_paddr()
    }

    /// Checks if the address space contains the given address range.
    pub fn contains_range(&self, start: VirtAddr, size: usize) -> bool {
        self.va_range
            .contains_range(VirtAddrRange::from_start_size(start, size))
    }

    /// Creates a new empty address space.
    pub fn new_empty(base: VirtAddr, size: usize) -> AxResult<Self> {
        Ok(Self {
            va_range: VirtAddrRange::from_start_size(base, size),
            areas: MemorySet::new(),
            pt: PageTable::try_new().map_err(|_| AxError::NoMemory)?,
        })
    }

    /// Copies page table mappings from another address space.
    ///
    /// It copies the page table entries only rather than the memory regions,
    /// usually used to copy a portion of the kernel space mapping to the
    /// user space.
    ///
    /// Note that on dropping, the copied PTEs will also be cleared, which could
    /// taint the original page table. For workaround, you can use
    /// [`AddrSpace::clear_mappings`].
    ///
    /// Returns an error if the two address spaces overlap.
    pub fn copy_mappings_from(&mut self, other: &AddrSpace) -> AxResult {
        if self.va_range.overlaps(other.va_range) {
            return ax_err!(InvalidInput, "address space overlap");
        }
        self.pt.copy_from(&other.pt, other.base(), other.size());
        Ok(())
    }

    /// Clears the page table mappings in the given address range.
    ///
    /// This should be used in pair with [`AddrSpace::copy_mappings_from`].
    pub fn clear_mappings(&mut self, range: VirtAddrRange) {
        self.pt.clear_copy_range(range.start, range.size());
    }

    /// The page table hardware can only map address ranges that are page-aligned.
    /// During the memory region validation in AddrSpace,
    /// the system enforces address alignment,
    /// ensuring that all memory operations comply with page boundary requirements.
    fn validate_region(&self, start: VirtAddr, size: usize, align: PageSize) -> AxResult {
        if !self.contains_range(start, size) {
            return ax_err!(InvalidInput, "address out of range");
        }
        if !start.is_aligned(align) || !is_aligned(size, align.into()) {
            return ax_err!(InvalidInput, "address not aligned");
        }
        Ok(())
    }

    /// Searches for a contiguous free region in the virtual address space
    ///
    /// This function searches for available virtual address space within a specified address range,
    /// based on the current memory region layout, that satisfies the size and alignment requirements.
    ///
    /// # Parameters
    /// - `hint`: Suggested starting address for the search (may be adjusted due to alignment or overlapping regions)
    /// - `size`: Size of the contiguous address space to allocate (in bytes)
    /// - `limit`: Boundary of the allowed address range (inclusive of start and end addresses)
    /// - `align`: Address alignment requirement (e.g., page alignment like 4KB/2MB)
    ///
    /// # Return Value
    /// - `Some(VirtAddr)`: A starting virtual address that meets all requirements was found
    /// - `None`: No sufficient space was found within the specified range
    ///
    /// # Implementation Logic
    /// 1. Initialize `last_end` to the maximum aligned value between the hint and the start of the limit range
    /// 2. First pass: handle regions before the hint to determine the initial search position
    /// 3. Second pass: check gaps between regions:
    ///    - Skip overlapping and already occupied regions
    ///    - Check whether the gap between regions satisfies the `size + alignment` requirement
    /// 4. Finally, verify that the found address is within the specified `limit` range
    ///
    /// # Notes
    /// - Alignment is strictly enforced on candidate addresses (ensured via `align_up`)
    /// - The region must be fully contained within the `limit` range (`end <= limit.end`)
    /// - The search may ignore the `hint` if a better space is found in later regions
    pub fn find_free_area(
        &self,
        hint: VirtAddr,
        size: usize,
        limit: VirtAddrRange,
        align: PageSize,
    ) -> Option<VirtAddr> {
        let mut last_end = hint.max(limit.start).align_up(align);
        for area in self.areas.iter() {
            if area.end() <= last_end {
                last_end = last_end.max(area.end().align_up(align));
            } else {
                break;
            }
        }
        for area in self.areas.iter() {
            let area_start = area.start();
            if area_start < last_end {
                continue;
            }
            if last_end
                .checked_add(size)
                .is_some_and(|end| end <= area_start)
            {
                return Some(last_end);
            }
            last_end = area.end().align_up(align);
        }

        if last_end
            .checked_add(size)
            .is_some_and(|end| end <= limit.end)
        {
            Some(last_end)
        } else {
            None
        }
    }

    /// Add a new linear mapping.
    ///
    /// See [`Backend`] for more details about the mapping backends.
    ///
    /// The `flags` parameter indicates the mapping permissions and attributes.
    ///
    /// Returns an error if the address range is out of the address space or not
    /// aligned.
    pub fn map_linear(
        &mut self,
        start_vaddr: VirtAddr,
        start_paddr: PhysAddr,
        size: usize,
        flags: MappingFlags,
        align: PageSize,
    ) -> AxResult {
        self.validate_region(start_vaddr, size, align)?;

        if !start_paddr.is_aligned(align) {
            return ax_err!(InvalidInput, "address not aligned");
        }

        let offset = start_vaddr.as_usize() - start_paddr.as_usize();
        let area = MemoryArea::new(start_vaddr, size, flags, Backend::new_linear(offset, align));
        self.areas
            .map(area, &mut self.pt, false)
            .map_err(mapping_err_to_ax_err)?;
        Ok(())
    }

    /// Add a new allocation mapping.
    ///
    /// See [`Backend`] for more details about the mapping backends.
    ///
    /// The `flags` parameter indicates the mapping permissions and attributes.
    ///
    /// Returns an error if the address range is out of the address space or not
    /// aligned.
    pub fn map_alloc(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        populate: bool,
        align: PageSize,
    ) -> AxResult {
        self.validate_region(start, size, align)?;

        let area = MemoryArea::new(start, size, flags, Backend::new_alloc(populate, align));
        self.areas
            .map(area, &mut self.pt, false)
            .map_err(mapping_err_to_ax_err)?;
        Ok(())
    }

    /// Ensures that the specified virtual memory region is fully mapped.
    ///
    /// This function walks through the given virtual address range and attempts to ensure
    /// that every page is mapped. If a page is not mapped and the corresponding area allows
    /// on-demand population (`populate == false`), it will trigger a page fault to map it.
    /// If `cow_on_write` is true, it will handle copy-on-write (COW) logic for already
    /// mapped pages that may require COW due to write intentions.
    ///
    /// # Parameters
    ///
    /// - `start`: The starting virtual address of the region to map.
    /// - `size`: The size (in bytes) of the region.
    /// - `align`: Alignment requirement for the allocated memory, must be a multiple of 4KiB.
    /// - `cow_on_write`: Whether to trigger copy-on-write handling for write-intended mappings.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the entire region is successfully mapped, or an appropriate
    /// `AxError` variant (`NoMemory`, `BadAddress`) on failure.
    ///
    /// # Errors
    ///
    /// - `AxError::NoMemory`: Failed to allocate.
    /// - `AxError::BadAddress`: An invalid mapping state was detected.
    pub fn ensure_region_mapped(
        &mut self,
        mut start: VirtAddr,
        size: usize,
        align: PageSize,
        cow_on_write: bool,
    ) -> AxResult {
        self.validate_region(start, size, align)?;
        let end = start + size;

        while let Some(area) = self.areas.find(start) {
            let backend = area.backend();
            if let Backend::Alloc { populate, align } = *backend {
                for addr in PageIterWrapper::new(start, area.end().min(end), align).unwrap() {
                    match self.pt.query(addr) {
                        // if the page is already mapped and write intentions, try cow.
                        Ok((paddr, flags, page_size)) => {
                            if cow_on_write {
                                if !area.flags().contains(MappingFlags::WRITE) {
                                    return Err(AxError::BadAddress);
                                }

                                if !Self::handle_cow_fault(
                                    addr,
                                    paddr,
                                    flags,
                                    page_size,
                                    &mut self.pt,
                                ) {
                                    return Err(AxError::NoMemory);
                                }
                            }
                        }
                        // If the page is not mapped, try map it.
                        Err(PagingError::NotMapped) => {
                            if !populate {
                                if !backend.handle_page_fault(addr, area.flags(), &mut self.pt) {
                                    return Err(AxError::NoMemory);
                                }
                            }
                        }
                        Err(_) => return Err(AxError::BadAddress),
                    };
                }
            }
            start = area.end();
            assert!(start.is_aligned(align));
            if start >= end {
                break;
            }
        }

        if start < end {
            // If the area is not fully mapped, we return ENOMEM.
            return ax_err!(NoMemory);
        }

        Ok(())
    }

    /// Removes mappings within the specified virtual address range.
    ///
    /// Returns an error if the address range is out of the address space or not
    /// aligned.
    pub fn unmap(&mut self, start: VirtAddr, size: usize) -> AxResult {
        self.validate_region(start, size, PageSize::Size4K)?;

        let end = start + size;
        for area in self
            .areas
            .iter()
            .skip_while(move |a| a.end() <= start)
            .take_while(move |a| a.start() < end)
        {
            let area_align = match *area.backend() {
                Backend::Alloc { populate: _, align } => align,
                Backend::Linear {
                    pa_va_offset: _,
                    align,
                } => align,
            };

            let unmap_start = start.max(area.start());
            let unmap_size = end.min(area.end()) - unmap_start;
            if !unmap_start.is_aligned(area_align) || !is_aligned(unmap_size, area_align.into()) {
                return ax_err!(InvalidInput, "address not aligned");
            }
        }

        self.areas
            .unmap(start, size, &mut self.pt)
            .map_err(mapping_err_to_ax_err)?;
        Ok(())
    }

    /// To remove user area mappings from address space.
    pub fn unmap_user_areas(&mut self) -> AxResult {
        self.areas.clear(&mut self.pt).unwrap();
        Ok(())
    }

    /// To process data in this area with the given function.
    ///
    /// Now it supports reading and writing data in the given interval.
    ///
    /// # Arguments
    /// - `start`: The start virtual address to process.
    /// - `size`: The size of the data to process.
    /// - `f`: The function to process the data, whose arguments are the start virtual address,
    ///   the offset and the size of the data.
    ///
    /// # Notes
    ///   The caller must ensure that the permission of the operation is allowed.
    fn process_area_data<F>(&self, start: VirtAddr, size: usize, align: PageSize, f: F) -> AxResult
    where
        F: FnMut(VirtAddr, usize, usize),
    {
        Self::process_area_data_with_page_table(&self.pt, &self.va_range, start, size, align, f)
    }

    fn process_area_data_with_page_table<F>(
        pt: &PageTable,
        va_range: &VirtAddrRange,
        start: VirtAddr,
        size: usize,
        align: PageSize,
        mut f: F,
    ) -> AxResult
    where
        F: FnMut(VirtAddr, usize, usize),
    {
        if !va_range.contains_range(VirtAddrRange::from_start_size(start, size)) {
            return ax_err!(InvalidInput, "address out of range");
        }
        let mut cnt = 0;
        // If start is aligned to 4K, start_align_down will be equal to start_align_up.
        let end_align_up = (start + size).align_up(align);
        let start_addr = start.align_down(align);
        for vaddr in PageIterWrapper::new(start_addr, end_align_up, align)
            .expect("Failed to create page iterator")
        {
            let (mut paddr, _, _) = pt.query(vaddr).map_err(|_| AxError::BadAddress)?;

            let mut copy_size = (size - cnt).min(PAGE_SIZE_4K);

            if copy_size == 0 {
                break;
            }
            if vaddr == start.align_down(align) && start.align_offset(align) != 0 {
                let align_offset = start.align_offset(align);
                copy_size = copy_size.min(align as usize - align_offset);
                paddr += align_offset;
            }
            f(phys_to_virt(paddr), cnt, copy_size);
            cnt += copy_size;
        }
        Ok(())
    }

    /// To read data from the address space.
    ///
    /// # Arguments
    ///
    /// * `start` - The start virtual address to read.
    /// * `buf` - The buffer to store the data.
    pub fn read(&self, start: VirtAddr, align: PageSize, buf: &mut [u8]) -> AxResult {
        self.process_area_data(start, buf.len(), align, |src, offset, read_size| unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), buf.as_mut_ptr().add(offset), read_size);
        })
    }

    /// To write data to the address space.
    ///
    /// # Arguments
    ///
    /// * `start_vaddr` - The start virtual address to write.
    /// * `buf` - The buffer to write to the address space.
    pub fn write(&self, start: VirtAddr, align: PageSize, buf: &[u8]) -> AxResult {
        self.process_area_data(start, buf.len(), align, |dst, offset, write_size| unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr().add(offset), dst.as_mut_ptr(), write_size);
        })
    }

    /// Updates mapping within the specified virtual address range.
    ///
    /// Returns an error if the address range is out of the address space or not
    /// aligned.
    pub fn protect(
        &mut self,
        start: VirtAddr,
        size: usize,
        flags: MappingFlags,
        align: PageSize,
    ) -> AxResult {
        // Populate the area first, which also checks the address range for us.
        self.ensure_region_mapped(start, size, align, false)?;

        self.areas
            .protect(start, size, |_| Some(flags), &mut self.pt)
            .map_err(mapping_err_to_ax_err)?;

        Ok(())
    }

    /// Removes all mappings in the address space.
    pub fn clear(&mut self) {
        self.areas.clear(&mut self.pt).unwrap();
    }

    /// Checks whether an access to the specified memory region is valid.
    ///
    /// Returns `true` if the memory region given by `range` is all mapped and
    /// has proper permission flags (i.e. containing `access_flags`).
    pub fn check_region_access(
        &self,
        mut range: VirtAddrRange,
        access_flags: MappingFlags,
    ) -> bool {
        for area in self.areas.iter() {
            if area.end() <= range.start {
                continue;
            }
            if area.start() > range.start {
                return false;
            }

            // This area overlaps with the memory region
            if !area.flags().contains(access_flags) {
                return false;
            }

            range.start = area.end();
            if range.is_empty() {
                return true;
            }
        }

        false
    }

    /// Handles a page fault at the given address.
    ///
    /// `access_flags` indicates the access type that caused the page fault.
    ///
    /// Returns `true` if the page fault is handled successfully (not a real
    /// fault).
    pub fn handle_page_fault(&mut self, vaddr: VirtAddr, access_flags: MappingFlags) -> bool {
        if !self.va_range.contains(vaddr) {
            return false;
        }
        if let Some(area) = self.areas.find(vaddr) {
            let orig_flags = area.flags();
            if orig_flags.contains(access_flags) {
                // Two cases enter the branch:
                // - shared pages (If there is a shared page in the vma)
                // - cow
                if let Ok((paddr, _, page_size)) = self.pt.query(vaddr) {
                    if !access_flags.contains(MappingFlags::WRITE) {
                        return false;
                    }
                    let off = page_size.align_offset(vaddr.into());
                    // 1. page fault caused by write
                    // 2. pte exists
                    // 3. Not shared memory
                    return Self::handle_cow_fault(
                        vaddr,
                        paddr.sub(off),
                        orig_flags,
                        page_size,
                        &mut self.pt,
                    );
                } else {
                    return area
                        .backend()
                        .handle_page_fault(vaddr, orig_flags, &mut self.pt);
                }
            }
        }
        false
    }

    /// Clone a [`AddrSpace`] by re-mapping all [`MemoryArea`]s in a new page table and copying data in user space.
    pub fn clone_or_err(&mut self) -> AxResult<Self> {
        let mut new_aspace = Self::new_empty(self.base(), self.size())?;

        for area in self.areas.iter() {
            let backend = area.backend();

            // Remap the memory area in the new address space.
            let new_area =
                MemoryArea::new(area.start(), area.size(), area.flags(), backend.clone());
            new_aspace
                .areas
                .map(new_area, &mut new_aspace.pt, false)
                .map_err(mapping_err_to_ax_err)?;

            if matches!(backend, Backend::Linear { .. }) {
                continue;
            }
            // Copy data from old memory area to new memory area.
            for vaddr in
                PageIter4K::new(area.start(), area.end()).expect("Failed to create page iterator")
            {
                let addr = match self.pt.query(vaddr) {
                    Ok((paddr, _, _)) => paddr,
                    // If the page is not mapped, skip it.
                    Err(PagingError::NotMapped) => continue,
                    Err(_) => return Err(AxError::BadAddress),
                };
                let new_addr = match new_aspace.pt.query(vaddr) {
                    Ok((paddr, _, _)) => paddr,
                    // If the page is not mapped, try map it.
                    Err(PagingError::NotMapped) => {
                        if !backend.handle_page_fault(vaddr, area.flags(), &mut new_aspace.pt) {
                            return Err(AxError::NoMemory);
                        }
                        match new_aspace.pt.query(vaddr) {
                            Ok((paddr, _, _)) => paddr,
                            Err(_) => return Err(AxError::BadAddress),
                        }
                    }
                    Err(_) => return Err(AxError::BadAddress),
                };
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        phys_to_virt(addr).as_ptr(),
                        phys_to_virt(new_addr).as_mut_ptr(),
                        PAGE_SIZE_4K,
                    )
                };
            }
        }
        Ok(new_aspace)
    }

    /// Creates a copy of the current [`AddrSpace`] with copy-on-write (COW)
    ///
    /// For pages that require COW, remove `write` flags.
    pub fn copy_with_cow(&mut self) -> AxResult<Self> {
        let mut new_aspace = Self::new_empty(self.base(), self.size())?;
        let new_pt = &mut new_aspace.pt;
        let old_pt = &mut self.pt;

        for area in self.areas.iter() {
            let mut backend = area.backend().clone();
            if let Backend::Alloc { populate, .. } = &mut backend {
                // Forcing `populate = false` is to prevent the subsequent `new_aspace.areas.map`
                // from mapping page table entries for the virtual addresses.
                *populate = false
            }

            let new_area =
                MemoryArea::new(area.start(), area.size(), area.flags(), backend.clone());
            new_aspace
                .areas
                .map(new_area, new_pt, false)
                .map_err(mapping_err_to_ax_err)?;

            // Linear-backed regions are usually allocated by the kernel and are shared
            if matches!(backend, Backend::Linear { .. }) {
                continue;
            }

            let mut flags = area.flags();
            flags.remove(MappingFlags::WRITE);

            //If the page is mapped in the old page table:
            // - Update its permissions in the old page table using `flags`.
            // - Map the same physical page into the new page table at the same
            // virtual address, with the same page size and `flags`.
            // TODO: huge page iter
            for vaddr in
                PageIter4K::new(area.start(), area.end()).expect("Failed to create page iterator")
            {
                if let Ok((paddr, _, page_size)) = old_pt.query(vaddr) {
                    // FIXME: need use huge page iter
                    add_frame_ref(paddr);

                    old_pt
                        .protect(vaddr, flags)
                        .map(|(_, tlb)| tlb.flush())
                        .expect("protect failed");
                    new_pt
                        .map(vaddr, paddr, page_size, flags)
                        .map(|tlb| tlb.flush())
                        .expect("map failed");
                }
            }
        }

        Ok(new_aspace)
    }

    /// Handles a Copy-On-Write (COW) page fault.
    ///
    /// # Arguments
    /// - `vaddr`: The virtual address that triggered the fault.
    /// - `paddr`: It must be an aligned physical address; if it's a huge page,
    /// it must be the starting physical address.
    /// - `flags`: vma flags.
    /// - `align`: Alignment requirement for the allocated memory, must be a multiple of 4KiB.
    /// - `pt`: A mutable reference to the page table that should be updated.
    ///
    /// # Returns
    /// - `true` if the page fault was handled successfully.
    /// - `false` if the fault handling failed (e.g., allocation failed or invalid ref count).
    fn handle_cow_fault(
        vaddr: VirtAddr,
        paddr: PhysAddr,
        flags: MappingFlags,
        align: PageSize,
        pt: &mut PageTable,
    ) -> bool {
        let frame_info = get_frame_info(paddr);
        match frame_info.ref_count() {
            0 => unreachable!(),
            // There is only one AddrSpace reference to the page,
            // so there is no need to copy it.
            1 => pt.protect(vaddr, flags).map(|(_, tlb)| tlb.flush()).is_ok(),
            // Allocates the new page and copies the contents of the original page,
            // remapping the virtual address to the physical address of the new page.
            2.. => match alloc_frame(false, align) {
                Some(new_frame) => {
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            phys_to_virt(paddr).as_ptr(),
                            phys_to_virt(new_frame).as_mut_ptr(),
                            align.into(),
                        )
                    };

                    dealloc_frame(paddr, align);

                    pt.remap(vaddr, new_frame, flags)
                        .map(|(_, tlb)| {
                            tlb.flush();
                        })
                        .is_ok()
                }
                None => false,
            },
        }
    }
}

impl fmt::Debug for AddrSpace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("AddrSpace")
            .field("va_range", &self.va_range)
            .field("page_table_root", &self.pt.root_paddr())
            .field("areas", &self.areas)
            .finish()
    }
}

impl Drop for AddrSpace {
    fn drop(&mut self) {
        self.clear();
    }
}
