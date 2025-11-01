use crate::serial_println;
use x86_64::structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB};
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

pub fn sys_gpu_map(user_addr: u64, _a1: u64, _a2: u64) -> u64 {
    serial_println!("GPU map request at 0x{:x}", user_addr);

    serial_println!("Acquiring GPU lock...");
    let gpu_guard = crate::GPU.lock();
    serial_println!("GPU lock acquired");

    serial_println!("Acquiring mapper lock...");
    let mut mapper_guard = crate::MAPPER.lock();
    serial_println!("Mapper lock acquired");

    serial_println!("Acquiring frame allocator lock...");
    let mut frame_allocator_guard = crate::FRAME_ALLOCATOR.lock();
    serial_println!("Frame allocator lock acquired");

    if let Some(gpu) = gpu_guard.as_ref() {
        let (fb_ptr, width, height) = gpu.get_framebuffer();
        let fb_phys = PhysAddr::new(fb_ptr as u64);
        let fb_size = (width * height * 4) as usize;
        let pages = (fb_size + 4095) / 4096;

        serial_println!(
            "Mapping {} pages from phys 0x{:x} to virt 0x{:x}",
            pages,
            fb_phys.as_u64(),
            user_addr
        );

        let mapper = mapper_guard.as_mut().unwrap();
        let frame_allocator = frame_allocator_guard.as_mut().unwrap();

        let flags = PageTableFlags::PRESENT
            | PageTableFlags::WRITABLE
            | PageTableFlags::USER_ACCESSIBLE
            | PageTableFlags::NO_CACHE;

        for i in 0..pages {
            if i % 100 == 0 {
                serial_println!("Mapping page {}/{}", i, pages);
            }

            let virt = VirtAddr::new(user_addr + (i * 4096) as u64);
            let phys = PhysAddr::new(fb_phys.as_u64() + (i * 4096) as u64);
            let page: Page<Size4KiB> = Page::containing_address(virt);
            let frame = PhysFrame::containing_address(phys);

            unsafe {
                match mapper.map_to(page, frame, flags, frame_allocator) {
                    Ok(flush) => flush.flush(),
                    Err(e) => {
                        serial_println!("Failed to map page {}: {:?}", i, e);
                        return u64::MAX;
                    }
                }
            }
        }

        serial_println!("Framebuffer mapped to userspace at 0x{:x}", user_addr);
        user_addr
    } else {
        serial_println!("GPU not initialized");
        u64::MAX
    }
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
