use alloc::{sync::Arc, vec::Vec};
use async_io::{Seek, SeekFrom};
use axalloc::PhysPage;
use axerrno::AxResult;
use axhal::{
    mem::{virt_to_phys, PhysAddr, VirtAddr, PAGE_SIZE_4K},
    paging::{MappingFlags, PageSize, PageTable},
};
use core::ptr::copy_nonoverlapping;
use sync::Mutex;

use crate::MemBackend;

/// A continuous virtual area in user memory.
///
/// NOTE: Cloning a `MapArea` needs allocating new phys pages and modifying a page table. So
/// `Clone` trait won't implemented.
pub struct MapArea {
    /// phys pages of this area
    pub pages: Vec<Option<Arc<Mutex<PhysPage>>>>,
    /// start virtual address
    pub vaddr: VirtAddr,
    /// shared in child process
    shared: bool,
    /// mapping flags of this area
    pub flags: MappingFlags,
    /// whether the area is backed by a file
    pub backend: Option<MemBackend>,
}

impl MapArea {
    /// Create a lazy-load area and map it in page table (page fault PTE).
    pub fn new_lazy(
        start: VirtAddr,
        num_pages: usize,
        flags: MappingFlags,
        backend: Option<MemBackend>,
        page_table: &mut PageTable,
    ) -> Self {
        let mut pages = Vec::with_capacity(num_pages);
        for _ in 0..num_pages {
            pages.push(None);
        }

        page_table
            .map_fault_region(start, num_pages * PAGE_SIZE_4K, flags)
            .unwrap();

        Self {
            pages,
            vaddr: start,
            shared: false,
            flags,
            backend,
        }
    }

    /// 在虚拟空间中分配一块不需要分配物理页的区域
    pub async fn new_without_alloc(
        start: VirtAddr,
        paddr: PhysAddr,
        num_pages: usize,
        flags: MappingFlags,
        page_table: &mut PageTable,
    ) -> AxResult<Self> {
        let mut pages = Vec::with_capacity(num_pages);
        for _ in 0..num_pages {
            pages.push(None);
        }

        page_table
            .map_region(start, paddr, num_pages * PAGE_SIZE_4K, flags, false)
            .unwrap();

        Ok(Self {
            pages,
            vaddr: start,
            shared: false,
            flags,
            backend: None,
        })
    }

    /// 在虚拟空间中分配一块不需要分配物理页的，标记为shared的区域
    pub async fn new_without_alloc_shared(
        start: VirtAddr,
        paddr: PhysAddr,
        num_pages: usize,
        flags: MappingFlags,
        page_table: &mut PageTable,
    ) -> AxResult<Self> {
        let mut pages = Vec::with_capacity(num_pages);
        for _ in 0..num_pages {
            pages.push(None);
        }

        page_table
            .map_region(start, paddr, num_pages * PAGE_SIZE_4K, flags, false)
            .unwrap();

        Ok(Self {
            pages,
            vaddr: start,
            shared: true,
            flags,
            backend: None,
        })
    }

    /// Allocated an area and map it in page table.
    pub async fn new_alloc(
        start: VirtAddr,
        num_pages: usize,
        flags: MappingFlags,
        data: Option<&[u8]>,
        backend: Option<MemBackend>,
        page_table: &mut PageTable,
    ) -> AxResult<Self> {
        let pages = PhysPage::alloc_contiguous(num_pages, PAGE_SIZE_4K, data)?
            .into_iter()
            .map(|page| page.map(|page| Arc::new(Mutex::new(page))))
            .collect::<Vec<_>>();

        debug!(
            "start: {:X?}, size: {:X},  page start: {:X?} flags: {:?}",
            start,
            num_pages * PAGE_SIZE_4K,
            pages[0].as_ref().unwrap().lock().await.start_vaddr,
            flags
        );
        page_table
            .map_region(
                start,
                virt_to_phys(pages[0].as_ref().unwrap().lock().await.start_vaddr),
                num_pages * PAGE_SIZE_4K,
                flags,
                false,
            )
            .unwrap();
        Ok(Self {
            pages,
            vaddr: start,
            shared: false,
            flags,
            backend,
        })
    }

    /// Set the shared flag of the area.
    pub(crate) fn set_shared(&mut self, shared: bool) {
        self.shared = shared;
    }

    /// Return whether the area is shared in child process.
    pub(crate) fn is_shared(&self) -> bool {
        self.shared
    }

    /// Deallocate all phys pages and unmap the area in page table.
    pub fn dealloc(&mut self, page_table: &mut PageTable) {
        page_table.unmap_region(self.vaddr, self.size()).unwrap();
        self.pages.clear();
    }

    /// 如果处理失败，返回false，此时直接退出当前程序
    pub async fn handle_page_fault(
        &mut self,
        addr: VirtAddr,
        flags: MappingFlags,
        page_table: &mut PageTable,
    ) -> bool {
        trace!(
            "handling {:?} page fault in area [{:?}, {:?})",
            addr,
            self.vaddr,
            self.end_va()
        );
        assert!(
            self.vaddr <= addr && addr < self.end_va(),
            "Try to handle page fault address out of bound"
        );
        if !self.flags.contains(flags) {
            error!(
                "Try to access {:?} memory addr: {:?} with {:?} flag",
                self.flags, addr, flags
            );
            return false;
        }

        let page_index = (usize::from(addr) - usize::from(self.vaddr)) / PAGE_SIZE_4K;
        if page_index >= self.pages.len() {
            error!("Phys page index out of bound");
            return false;
        }
        if self.pages[page_index].is_some() {
            debug!("Page fault in page already loaded");
            return true;
        }

        debug!("page index {}", page_index);

        // Allocate new page
        let mut page = PhysPage::alloc().expect("Error allocating new phys page for page fault");

        debug!(
            "new phys page virtual (offset) address {:?}",
            page.start_vaddr
        );

        // Read data from backend to fill with 0.
        match &mut self.backend {
            Some(backend) => {
                if backend
                    .read_from_seek(
                        SeekFrom::Current((page_index * PAGE_SIZE_4K) as i64),
                        page.as_slice_mut(),
                    )
                    .await
                    .is_err()
                {
                    warn!("Failed to read from backend to memory");
                    page.fill(0);
                }
            }
            None => page.fill(0),
        };

        // Map newly allocated page in the page_table
        page_table
            .map_overwrite(
                addr.align_down_4k(),
                virt_to_phys(page.start_vaddr),
                axhal::paging::PageSize::Size4K,
                self.flags,
            )
            .expect("Map in page fault handler failed");

        axhal::arch::flush_tlb(addr.align_down_4k().into());
        self.pages[page_index] = Some(Arc::new(Mutex::new(page)));
        true
    }

    /// Sync pages in index back to `self.backend` (if there is one).
    ///
    /// # Panics
    ///
    /// Panics if index is out of bounds.
    pub async fn sync_page_with_backend(&mut self, page_index: usize) {
        if let Some(page) = &self.pages[page_index] {
            if let Some(backend) = &mut self.backend {
                if backend.writable().await {
                    let _ = backend
                        .write_to_seek(
                            SeekFrom::Start((page_index * PAGE_SIZE_4K) as u64),
                            page.lock().await.as_slice(),
                        )
                        .await
                        .unwrap();
                }
            }
        } else {
            debug!("Tried to sync an unallocated page");
        }
    }

    /// Deallocate some pages from the start of the area.
    /// This function will unmap them in a page table. You need to flush TLB after this function.
    pub async fn shrink_left(&mut self, new_start: VirtAddr, page_table: &mut PageTable) {
        assert!(new_start.is_aligned_4k());

        let delete_size = new_start.as_usize() - self.vaddr.as_usize();
        let delete_pages = delete_size / PAGE_SIZE_4K;

        // move backend offset
        if let Some(backend) = &mut self.backend {
            let _ = backend
                .seek(SeekFrom::Current(delete_size as i64))
                .await
                .unwrap();
        }

        // remove (dealloc) phys pages
        drop(self.pages.drain(0..delete_pages));

        // unmap deleted pages
        page_table.unmap_region(self.vaddr, delete_size).unwrap();

        self.vaddr = new_start;
    }

    /// Deallocate some pages from the end of the area.
    /// This function will unmap them in a page table. You need to flush TLB after this function.
    pub fn shrink_right(&mut self, new_end: VirtAddr, page_table: &mut PageTable) {
        assert!(new_end.is_aligned_4k());

        let delete_size = self.end_va().as_usize() - new_end.as_usize();
        let delete_pages = delete_size / PAGE_SIZE_4K;

        // remove (dealloc) phys pages
        drop(
            self.pages
                .drain((self.pages.len() - delete_pages)..self.pages.len()),
        );

        // unmap deleted pages
        page_table.unmap_region(new_end, delete_size).unwrap();
    }

    /// Split this area into 2.
    pub async fn split(&mut self, addr: VirtAddr) -> Self {
        assert!(addr.is_aligned_4k());

        let right_page_count = (self.end_va() - addr.as_usize()).as_usize() / PAGE_SIZE_4K;
        let right_page_range = self.pages.len() - right_page_count..self.pages.len();

        let right_pages = self.pages.drain(right_page_range).collect();

        let backend = if let Some(backend) = self.backend.as_ref() {
            let mut backend = backend.clone();
            let _ = backend
                .seek(SeekFrom::Current(
                    (addr.as_usize() - self.vaddr.as_usize()) as i64,
                ))
                .await
                .unwrap();
            Some(backend)
        } else {
            None
        };
        Self {
            pages: right_pages,
            vaddr: addr,
            flags: self.flags,
            shared: self.shared,
            backend,
        }
    }

    /// Split this area into 3.
    pub async fn split3(&mut self, start: VirtAddr, end: VirtAddr) -> (Self, Self) {
        assert!(start.is_aligned_4k());
        assert!(end.is_aligned_4k());
        assert!(start < end);
        assert!(self.vaddr < start);
        assert!(end < self.end_va());

        let right_pages = self
            .pages
            .drain(
                self.pages.len() - (self.end_va().as_usize() - end.as_usize()) / PAGE_SIZE_4K
                    ..self.pages.len(),
            )
            .collect();

        let mid_pages = self
            .pages
            .drain(
                self.pages.len() - (self.end_va().as_usize() - start.as_usize()) / PAGE_SIZE_4K
                    ..self.pages.len(),
            )
            .collect();

        let mid_backend = if let Some(backend) = self.backend.as_ref() {
            let mut backend = backend.clone();
            let _ = backend
                .seek(SeekFrom::Current(
                    (start.as_usize() - self.vaddr.as_usize()) as i64,
                ))
                .await
                .unwrap();
            Some(backend)
        } else {
            None
        };

        let mid = Self {
            pages: mid_pages,
            vaddr: start,
            flags: self.flags,
            shared: self.shared,
            backend: mid_backend,
        };

        let right_backend = if let Some(backend) = self.backend.as_ref() {
            let mut backend = backend.clone();
            let _ = backend
                .seek(SeekFrom::Current(
                    (end.as_usize() - self.vaddr.as_usize()) as i64,
                ))
                .await
                .unwrap();
            Some(backend)
        } else {
            None
        };

        let right = Self {
            pages: right_pages,
            vaddr: end,
            flags: self.flags,
            shared: self.shared,
            backend: right_backend,
        };

        (mid, right)
    }

    /// Create a second area in the right part of the area, [self.vaddr, left_end) and
    /// [right_start, self.end_va()).
    /// This function will unmap deleted pages in a page table. You need to flush TLB after calling
    /// this.
    pub async fn remove_mid(
        &mut self,
        left_end: VirtAddr,
        right_start: VirtAddr,
        page_table: &mut PageTable,
    ) -> Self {
        assert!(left_end.is_aligned_4k());
        assert!(right_start.is_aligned_4k());
        // We can have left_end == right_start, although it doesn't do anything other than create
        // two areas.
        assert!(left_end <= right_start);

        let delete_size = right_start.as_usize() - left_end.as_usize();
        let delete_range = ((left_end.as_usize() - self.vaddr.as_usize()) / PAGE_SIZE_4K)
            ..((right_start.as_usize() - self.vaddr.as_usize()) / PAGE_SIZE_4K);

        // create a right area
        let pages = self
            .pages
            .drain(((right_start.as_usize() - self.vaddr.as_usize()) / PAGE_SIZE_4K)..)
            .collect();

        let right_backend = if let Some(backend) = self.backend.as_ref() {
            let mut backend = backend.clone();
            let _ = backend
                .seek(SeekFrom::Current(
                    (right_start.as_usize() - self.vaddr.as_usize()) as i64,
                ))
                .await
                .unwrap();
            Some(backend)
        } else {
            None
        };

        let right_area = Self {
            pages,
            vaddr: right_start,
            flags: self.flags,
            shared: self.shared,
            backend: right_backend,
        };

        // remove pages
        let _ = self.pages.drain(delete_range);

        page_table.unmap_region(left_end, delete_size).unwrap();

        right_area
    }
}

impl MapArea {
    /// return the size of the area, which thinks the page size is default 4K.
    pub fn size(&self) -> usize {
        self.pages.len() * PAGE_SIZE_4K
    }

    /// return the end virtual address of the area.
    pub fn end_va(&self) -> VirtAddr {
        self.vaddr + self.size()
    }

    /// return whether all the pages have been allocated.
    pub fn allocated(&self) -> bool {
        self.pages.iter().all(|page| page.is_some())
    }
    /// # Safety
    /// This function is unsafe because it dereferences a raw pointer.
    /// It will return a slice of the area's memory, whose len is the same as the area's size.
    pub unsafe fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.vaddr.as_ptr(), self.size()) }
    }

    /// Fill `self` with `byte`.
    pub async fn fill(&mut self, byte: u8) {
        for page in self.pages.iter_mut() {
            if let Some(page) = page {
                page.lock().await.fill(byte);
            }
        }
    }

    /// If [start, end) overlaps with self.
    pub fn overlap_with(&self, start: VirtAddr, end: VirtAddr) -> bool {
        self.vaddr <= start && start < self.end_va() || start <= self.vaddr && self.vaddr < end
    }

    /// If [start, end] contains self.
    pub fn contained_in(&self, start: VirtAddr, end: VirtAddr) -> bool {
        start <= self.vaddr && self.end_va() <= end
    }

    /// If self contains [start, end].
    pub fn contains(&self, start: VirtAddr, end: VirtAddr) -> bool {
        self.vaddr <= start && end <= self.end_va()
    }

    /// If self strictly contains [start, end], which stands for the start and end are not equal to self's.
    pub fn strict_contain(&self, start: VirtAddr, end: VirtAddr) -> bool {
        self.vaddr < start && end < self.end_va()
    }

    /// Update area's mapping flags and write it to page table. You need to flush TLB after calling
    /// this function.
    pub fn update_flags(&mut self, flags: MappingFlags, page_table: &mut PageTable) {
        self.flags = flags;
        page_table
            .update_region(self.vaddr, self.size(), flags)
            .unwrap();
    }
    /// # Clone the area.
    ///
    /// If the area is shared, we don't need to allocate new phys pages.
    ///
    /// If the area is not shared and all the pages have been allocated,
    /// we can allocate a contiguous area in phys memory.
    ///
    /// This function will modify the page table as well.
    ///
    /// # Arguments
    ///
    /// * `page_table` - The page table of the new child process.
    ///
    /// * `parent_page_table` - The page table of the current process.
    pub async fn clone_alloc(
        &mut self,
        page_table: &mut PageTable,
        parent_page_table: &mut PageTable,
    ) -> AxResult<Self> {
        // If the area is shared, we don't need to allocate new phys pages.
        if self.is_shared() {
            // Allocated all fault page in the parent page table.
            let fault_pages: Vec<_> = self
                .pages
                .iter()
                .enumerate()
                .filter_map(|(idx, slot)| {
                    if slot.is_none() {
                        Some(self.vaddr + (idx * PAGE_SIZE_4K))
                    } else {
                        None
                    }
                })
                .collect();
            for vaddr in fault_pages {
                if let Ok((pa, _, _)) = parent_page_table.query(vaddr) {
                    if pa.as_usize() != 0 {
                        // 用于排除pages为[None]，但page_table中已有映射关系的情况（用于vDSO实现），此时不需要handle_page_fault
                        continue;
                    }
                }
                self.handle_page_fault(vaddr, MappingFlags::empty(), parent_page_table)
                    .await;
            }

            // Map the area in the child page table.
            let mut pages = Vec::new();
            for (idx, slot) in self.pages.iter().enumerate() {
                let vaddr = self.vaddr + (idx * PAGE_SIZE_4K);
                // assert!(slot.is_some());
                if slot.is_some() {
                    let page = slot.as_ref().unwrap().lock().await;
                    page_table
                        .map(
                            vaddr,
                            virt_to_phys(page.start_vaddr),
                            PageSize::Size4K,
                            self.flags,
                        )
                        .unwrap();
                    drop(page);
                    pages.push(Some(Arc::clone(slot.as_ref().unwrap())));
                } else {
                    // 增加了pages为[None]，但page_table中已有映射关系的情况（用于vDSO实现）
                    let paddr = parent_page_table.query(vaddr).unwrap().0;
                    assert!(paddr.as_usize() != 0);
                    page_table
                        .map(vaddr, paddr, PageSize::Size4K, self.flags)
                        .unwrap();
                    pages.push(None);
                }
            }
            return Ok(Self {
                pages,
                vaddr: self.vaddr,
                flags: self.flags,
                shared: self.shared,
                backend: self.backend.clone(),
            });
        }
        // All the pages have been allocated. Allocate a contiguous area in phys memory.
        if self.allocated() {
            MapArea::new_alloc(
                self.vaddr,
                self.pages.len(),
                self.flags,
                Some(unsafe { self.as_slice() }),
                self.backend.clone(),
                page_table,
            )
            .await
        } else {
            let mut pages = Vec::new();
            for (idx, slot) in self.pages.iter().enumerate() {
                let vaddr = self.vaddr + (idx * PAGE_SIZE_4K);
                match slot.as_ref() {
                    Some(page) => {
                        let mut new_page = PhysPage::alloc().unwrap();
                        unsafe {
                            copy_nonoverlapping(
                                page.lock().await.as_ptr(),
                                new_page.as_mut_ptr(),
                                PAGE_SIZE_4K,
                            );
                        }

                        page_table
                            .map(
                                vaddr,
                                virt_to_phys(new_page.start_vaddr),
                                PageSize::Size4K,
                                self.flags,
                            )
                            .unwrap();

                        pages.push(Some(Arc::new(Mutex::new(new_page))));
                    }
                    None => {
                        page_table
                            .map_fault(vaddr, PageSize::Size4K, self.flags)
                            .unwrap();
                        pages.push(None);
                    }
                }
            }
            Ok(Self {
                pages,
                vaddr: self.vaddr,
                flags: self.flags,
                shared: self.shared,
                backend: self.backend.clone(),
            })
        }
    }
}
