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
    use sight::Rect;
    let fb = VirtioFramebuffer::new(gpu, mapper, frame_allocator);
    let mut ctx = Sight::new(fb);

    ctx.clear(Color::BLACK);

    ctx.draw_circle(Point::new(200, 200), 80, Color::RED);
    ctx.draw_rect(Rect::new(400, 150, 150, 100), Color::BLUE);
    ctx.draw_rounded_rect(Rect::new(600, 150, 150, 120), 20, Color::GREEN);

    ctx.draw_triangle(
        Point::new(100, 500),
        Point::new(250, 400),
        Point::new(200, 550),
        Color::YELLOW,
    );

    ctx.draw_line(Point::new(50, 50), Point::new(300, 350), Color::WHITE);
    ctx.draw_line(Point::new(800, 50), Point::new(950, 300), Color::CYAN);

    ctx.fill_gradient_h(Rect::new(0, 650, 1024, 117), Color::PURPLE, Color::CYAN);

    ctx.present().unwrap();
}
