use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB,
    },
    PhysAddr, VirtAddr,
};

pub unsafe fn init(
    physical_memory_offset: VirtAddr,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> OffsetPageTable<'static> {
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        let mut mapper = OffsetPageTable::new(level_4_table, physical_memory_offset);

        map_apic(&mut mapper, frame_allocator);

        mapper
    }
}

unsafe fn map_apic(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    const APIC_BASE: u64 = 0xFEE0_0000;

    let apic_page: Page = Page::containing_address(VirtAddr::new(APIC_BASE));
    let apic_frame = PhysFrame::containing_address(PhysAddr::new(APIC_BASE));
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;

    unsafe {
        mapper
            .map_to(apic_page, apic_frame, flags, frame_allocator)
            .expect("Failed to map APIC")
            .flush();
    }
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

pub struct EmptyFrameAllocator;

unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    current_region: usize,
    current_frame_in_region: u64,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        let mut allocator = BootInfoFrameAllocator {
            memory_map,
            current_region: 0,
            current_frame_in_region: 0,
        };

        const MIN_FRAME_ADDR: u64 = 0x1000000;

        for (idx, region) in memory_map.iter().enumerate() {
            if region.region_type == MemoryRegionType::Usable {
                if region.range.end_addr() > MIN_FRAME_ADDR {
                    allocator.current_region = idx;

                    if region.range.start_addr() < MIN_FRAME_ADDR {
                        let skip_bytes = MIN_FRAME_ADDR - region.range.start_addr();
                        allocator.current_frame_in_region = skip_bytes / 4096;
                    }

                    break;
                }
            }
        }

        allocator
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        loop {
            let region = self.memory_map.iter().nth(self.current_region)?;

            if region.region_type != MemoryRegionType::Usable {
                self.current_region += 1;
                self.current_frame_in_region = 0;
                continue;
            }

            let frame_addr = region.range.start_addr() + (self.current_frame_in_region * 4096);

            if frame_addr < 0x1000000 {
                self.current_frame_in_region += 1;
                continue;
            }

            if frame_addr >= region.range.end_addr() {
                self.current_region += 1;
                self.current_frame_in_region = 0;
                continue;
            }

            self.current_frame_in_region += 1;
            return Some(PhysFrame::containing_address(PhysAddr::new(frame_addr)));
        }
    }
}
