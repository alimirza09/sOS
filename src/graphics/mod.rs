use crate::{drivers::pci::VirtioGpu, serial_println, syscall::SYS_READ};
use sight::{bdf::parse_bdf_font, bmp, Color, Framebuffer, Point, Sight};
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

    let filename = b"IMAGE.BMP\0";
    let fd = crate::syscall::syscall_identifier(
        crate::syscall::SYS_OPEN,
        filename.as_ptr() as u64,
        0,
        0,
    ) as i64;

    if fd < 0 {
        serial_println!("failed to open file");
    } else {
        let file_size = 786486;
        let mut buf = alloc::vec![0u8; file_size];

        let bytes_read = crate::syscall::syscall_identifier(
            SYS_READ,
            fd as u64,
            buf.as_mut_ptr() as u64,
            file_size as u64,
        ) as i64;

        if bytes_read > 0 {
            serial_println!("read {} bytes", bytes_read);

            let image = sight::bmp::BmpImage::from_bytes(&buf[..bytes_read as usize]);
            match image {
                Ok(image) => {
                    serial_println!("bmp loaded {}x{}", image.width, image.height);
                    ctx.draw_bmp(&image, 100, 100);
                }
                Err(e) => {
                    serial_println!("bmp error {}", e);
                }
            }
        }
    }

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

pub fn test_sight_text<'a, FA: FrameAllocator<Size4KiB>>(
    gpu: &'a mut VirtioGpu,
    mapper: &'a mut OffsetPageTable<'a>,
    frame_allocator: &'a mut FA,
) {
    let fb = VirtioFramebuffer::new(gpu, mapper, frame_allocator);
    let mut ctx = Sight::new(fb);
    ctx.clear(Color::BLACK);

    let filename = b"FONT.BDF\0";
    let fd = crate::syscall::syscall_identifier(
        crate::syscall::SYS_OPEN,
        filename.as_ptr() as u64,
        0,
        0,
    ) as i64;

    if fd < 0 {
        serial_println!("failed to open file");
    } else {
        let file_size = 2097152;
        let mut buf = alloc::vec![0u8; file_size];

        let bytes_read = crate::syscall::syscall_identifier(
            SYS_READ,
            fd as u64,
            buf.as_mut_ptr() as u64,
            file_size as u64,
        ) as i64;

        if bytes_read > 0 {
            serial_println!("read {} bytes", bytes_read);
            match parse_bdf_font(&buf[..bytes_read as usize]) {
                Ok(font) => {
                    serial_println!("font loaded {} glyphs", font.glyphs.len());
                    serial_println!("font bbox {}x{}", font.bounding_box.0, font.bounding_box.1);

                    // Test if 'H' exists
                    if let Some(glyph) = font.get_glyph('H') {
                        serial_println!("found H glyph: {}x{}", glyph.width, glyph.height);
                    } else {
                        serial_println!("H glyph missing");
                    }

                    font.draw_text("Hello World", 100, 100, |x, y| {
                        serial_println!("drawing pixel at {},{}", x, y); // Add this
                        ctx.put_pixel(x, y, Color::GREEN);
                    });
                }
                Err(e) => {
                    serial_println!("font parse error {}", e);
                }
            }
        }
    }

    ctx.present().unwrap();
}
