#![no_std]
#![no_main]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

use sos::drivers::vga_buffer::{set_colors, Color};
use sos::{println, serial_println};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    set_colors(Color::Green, Color::Black);
    println!("Welcome to sOS!");
    serial_println!("Welcome to sOS!");
    let (mut frame_allocator, mut mapper) = sos::init(boot_info);

    if let Some(gpu_dev) = sos::drivers::pci::find_virtio_gpu() {
        serial_println!("Initializing VirtIO-GPU");

        let mut gpu = sos::drivers::pci::VirtioGpu::new(gpu_dev);

        match gpu.init(&mut mapper, &mut frame_allocator) {
            Ok(()) => {
                serial_println!("VirtIO-GPU initialized.");

                let (fb_ptr, width, height) = gpu.get_framebuffer();
                serial_println!("Framebuffer ready: {}x{} at {:p}", width, height, fb_ptr);

                match gpu.refresh_display(&mut mapper, &mut frame_allocator) {
                    Ok(()) => {
                        serial_println!("Display refreshed")
                    }
                    Err(e) => serial_println!("Failed to refresh display: {}", e),
                }
                gpu.debug_and_refresh();
            }
            Err(e) => {
                serial_println!("Failed to initialize VirtIO-GPU: {}", e);
            }
        }
    } else {
        serial_println!("No VirtIO-GPU device found");
    }
    serial_println!("==================================");

    sos::ata::test_ata_driver_comprehensive();
    sos::fs::fat::test_fat32_with_device(sos::ata::AtaDevice::Slave, 131072);
    sos::syscall::test_syscalls();

    serial_println!("==================================");

    sos::elf::loader::run_elf_exec("hello.elf");

    serial_println!("Entering an infinite loop.");
    sos::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("=== KERNEL PANIC ===");
    serial_println!("PANIC: {}", info);

    if let Some(location) = info.location() {
        serial_println!(
            "Panic occurred in file '{}' at line {}",
            location.file(),
            location.line()
        );
    }

    let message = info.message();
    serial_println!("Panic message: {}", message);

    serial_println!("System halted due to panic - entering infinite loop");

    sos::hlt_loop();
}
