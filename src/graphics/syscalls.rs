use crate::serial_println;
use x86_64::structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB, Translate};
use x86_64::{PhysAddr, VirtAddr};

pub fn sys_gpu_info(_a0: u64, _a1: u64, _a2: u64) -> u64 {
    let gpu_guard = crate::GPU.lock();
    if let Some(gpu) = gpu_guard.as_ref() {
        let (_, width, height) = gpu.get_framebuffer();
        serial_println!("GPU info: {}x{}", width, height);
        (width as u64) | ((height as u64) << 32)
    } else {
        serial_println!("GPU not initialized");
        u64::MAX
    }
}

use x86_64::instructions::interrupts;

const PHYS_BITS: u64 = 52;
const PHYS_MASK: u64 = (1u64 << PHYS_BITS) - 1;
const PHYS_OFFSET: u64 = 0x18000000000;
const PAGE_SIZE: usize = 4096;
const BATCH_PAGES: usize = 64;

pub fn sys_gpu_map(user_addr: u64, _a1: u64, _a2: u64) -> u64 {
    serial_println!("GPU map request at 0x{:x}", user_addr);

    serial_println!("Acquiring GPU lock...");
    let gpu_guard = crate::GPU.lock();
    serial_println!("GPU lock acquired");

    let fb_info = if let Some(gpu) = gpu_guard.as_ref() {
        let (fb_ptr, width, height) = gpu.get_framebuffer();
        Some((fb_ptr as u64, width as usize, height as usize))
    } else {
        serial_println!("GPU not initialized");
        return u64::MAX;
    };

    drop(gpu_guard);

    let (fb_virt_u, width, height) = fb_info.unwrap();
    let fb_size = width.saturating_mul(height).saturating_mul(4);
    let pages = (fb_size + PAGE_SIZE - 1) / PAGE_SIZE;
    serial_println!(
        "Framebuffer virt 0x{:x}, size {} bytes ({} pages)",
        fb_virt_u,
        fb_size,
        pages
    );

    serial_println!("Acquiring mapper lock to take ownership...");
    let mut mapper_opt = {
        let mut g = crate::MAPPER.lock();
        g.take()
    };
    serial_println!("Mapper taken");

    serial_println!("Acquiring frame allocator lock to take ownership...");
    let mut fa_opt = {
        let mut g = crate::FRAME_ALLOCATOR.lock();
        g.take()
    };
    serial_println!("Frame allocator taken");

    fn restore_globals(
        mapper_opt: Option<crate::OffsetPageTable<'static>>,
        fa_opt: Option<crate::BootInfoFrameAllocator>,
    ) {
        if let Some(mapper) = mapper_opt {
            let mut g = crate::MAPPER.lock();
            *g = Some(mapper);
        }
        if let Some(fa) = fa_opt {
            let mut g = crate::FRAME_ALLOCATOR.lock();
            *g = Some(fa);
        }
    }

    if mapper_opt.is_none() || fa_opt.is_none() {
        serial_println!("Mapper or frame allocator missing");
        restore_globals(mapper_opt, fa_opt);
        return u64::MAX;
    }

    let mut mapper = mapper_opt.take().unwrap();
    let mut frame_allocator = fa_opt.take().unwrap();

    let fb_phys_u: u64 = {
        if let Some(pa) = mapper.translate_addr(VirtAddr::new(fb_virt_u)) {
            let pa_u = pa.as_u64();
            if (pa_u >> PHYS_BITS) != 0 {
                serial_println!(
                    "Warning: translate_addr returned non-canonical phys 0x{:x}; masking to 52 bits",
                    pa_u
                );
                pa_u & PHYS_MASK
            } else {
                pa_u
            }
        } else {
            if fb_virt_u >= PHYS_OFFSET {
                let candidate = fb_virt_u.wrapping_sub(PHYS_OFFSET);
                if (candidate >> PHYS_BITS) != 0 {
                    serial_println!(
                        "Framebuffer fallback phys 0x{:x} invalid after subtracting PHYS_OFFSET",
                        candidate
                    );

                    restore_globals(Some(mapper), Some(frame_allocator));
                    return u64::MAX;
                }
                serial_println!(
                    "translate_addr failed; using fallback phys 0x{:x} (virt 0x{:x} - offset)",
                    candidate,
                    fb_virt_u
                );
                candidate
            } else {
                serial_println!(
                    "translate_addr failed and fb virt 0x{:x} < PHYS_OFFSET; cannot resolve",
                    fb_virt_u
                );
                restore_globals(Some(mapper), Some(frame_allocator));
                return u64::MAX;
            }
        }
    };

    if (fb_phys_u >> PHYS_BITS) != 0 {
        serial_println!(
            "Resolved framebuffer physical 0x{:x} is not canonical",
            fb_phys_u
        );
        restore_globals(Some(mapper), Some(frame_allocator));
        return u64::MAX;
    }

    serial_println!(
        "Mapping {} pages from phys 0x{:x} to virt 0x{:x}",
        pages,
        fb_phys_u,
        user_addr
    );

    let flags =
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
    let mut page_index: usize = 0;
    let mut map_failed = false;

    while page_index < pages {
        let batch_end = core::cmp::min(page_index + BATCH_PAGES, pages);

        interrupts::without_interrupts(|| {
            for i in page_index..batch_end {
                let virt = VirtAddr::new(user_addr.wrapping_add((i * PAGE_SIZE) as u64));
                let phys_addr_u = fb_phys_u.wrapping_add((i * PAGE_SIZE) as u64);

                if (phys_addr_u >> PHYS_BITS) != 0 {
                    map_failed = true;
                    return;
                }

                let phys = PhysAddr::new(phys_addr_u);
                let page: Page<Size4KiB> = Page::containing_address(virt);
                let frame = PhysFrame::containing_address(phys);

                unsafe {
                    match mapper.map_to(page, frame, flags, &mut frame_allocator) {
                        Ok(flush) => flush.flush(),
                        Err(e) => {
                            serial_println!("Failed to map page {}: {:?}", i, e);
                            map_failed = true;
                            return;
                        }
                    }
                }
            }
        });

        if map_failed {
            serial_println!("Mapping failed at pages {}..{}", page_index, batch_end);
            break;
        }

        if page_index % (BATCH_PAGES * 2) == 0 {
            serial_println!("Mapped up to page {}/{}", batch_end, pages);
        }

        page_index = batch_end;
    }

    if map_failed {
        restore_globals(Some(mapper), Some(frame_allocator));
        return u64::MAX;
    }

    {
        let mut g = crate::MAPPER.lock();
        *g = Some(mapper);
    }
    {
        let mut g = crate::FRAME_ALLOCATOR.lock();
        *g = Some(frame_allocator);
    }

    serial_println!("Framebuffer mapped to userspace at 0x{:x}", user_addr);
    user_addr
}

pub fn sys_gpu_flush(_a0: u64, _a1: u64, _a2: u64) -> u64 {
    serial_println!("GPU flush request");

    let mut gpu_guard = crate::GPU.lock();
    let mut mapper_guard = crate::MAPPER.lock();
    let mut frame_allocator_guard = crate::FRAME_ALLOCATOR.lock();

    if let Some(gpu) = gpu_guard.as_mut() {
        let mapper = mapper_guard.as_mut().unwrap();
        let frame_allocator = frame_allocator_guard.as_mut().unwrap();

        match gpu.refresh_display(mapper, frame_allocator) {
            Ok(_) => {
                serial_println!("GPU flushed successfully");
                0
            }
            Err(e) => {
                serial_println!("GPU flush failed: {}", e);
                u64::MAX
            }
        }
    } else {
        serial_println!("GPU not initialized");
        u64::MAX
    }
}
