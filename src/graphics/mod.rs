use crate::drivers::pci::VirtioGpu;
use sight::{Color, Framebuffer, Point, Sight};
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, Size4KiB};

pub struct VirtioFramebuffer<'a, FA: FrameAllocator<Size4KiB>> {
    gpu: &'a mut VirtioGpu,
    fb: *mut u32,
    width: u32,
    height: u32,
    mapper: &'a mut OffsetPageTable<'a>,
    frame_allocator: &'a mut FA,
}

impl<'a, FA: FrameAllocator<Size4KiB>> VirtioFramebuffer<'a, FA> {
    pub fn new(
        gpu: &'a mut VirtioGpu,
        mapper: &'a mut OffsetPageTable<'a>,
        frame_allocator: &'a mut FA,
    ) -> Self {
        let (fb, width, height) = gpu.get_framebuffer();
        Self {
            gpu,
            fb,
            width,
            height,
            mapper,
            frame_allocator,
        }
    }
}

impl<'a, FA: FrameAllocator<Size4KiB>> Framebuffer for VirtioFramebuffer<'a, FA> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    unsafe fn write_pixel(&mut self, x: u32, y: u32, color: u32) -> bool {
        if x < self.width && y < self.height {
            let offset = (y * self.width + x) as usize;
            *self.fb.add(offset) = color;
            true
        } else {
            false
        }
    }

    fn flush(&mut self) -> Result<(), &'static str> {
        self.gpu.refresh_display(self.mapper, self.frame_allocator)
    }
}

pub fn test_sight<'a, FA: FrameAllocator<Size4KiB>>(
    gpu: &'a mut VirtioGpu,
    mapper: &'a mut OffsetPageTable<'a>,
    frame_allocator: &'a mut FA,
) {
    let fb = VirtioFramebuffer::new(gpu, mapper, frame_allocator);
    let mut ctx = Sight::new(fb);

    ctx.clear(Color::BLACK);
    ctx.draw_line(Point::new(0, 0), Point::new(1023, 767), Color::RED);
    ctx.present().unwrap();
}
