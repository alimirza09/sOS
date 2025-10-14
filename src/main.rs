#![no_std]
#![no_main]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use sos::ata::AtaDevice;

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

    let drive_info = match sos::ata::identify_drive(true, AtaDevice::Slave) {
        Ok(info) => {
            crate::serial_println!("Primary Slave found:");
            crate::serial_println!("  Model: {}", info.model);
            crate::serial_println!("  Serial: {}", info.serial);
            crate::serial_println!("  Firmware: {}", info.firmware);
            crate::serial_println!("  Sectors: {}", info.sectors);
            crate::serial_println!(
                "  Capacity: {} MB ({} GB)",
                info.capacity_mb(),
                info.capacity_gb()
            );
            crate::serial_println!("  LBA48 Support: {}", info.supports_lba48);
            crate::serial_println!("  Sector Size: {} bytes", info.sector_size);
            Some(info)
        }
        Err(e) => {
            crate::serial_println!("Primary Slave error: {:?}", e);
            None
        }
    };

    sos::fs::fat::mount_root_fs(
        sos::ata::AtaDevice::Slave,
        drive_info.unwrap().sectors as u32,
    );
    if let Ok(some) = sos::fs::fat::list_dir("") {
        serial_println!("{:?}", some);
    }

    serial_println!("==================================");

    sos::elf::runner::run_elf_in_kernel_mode("hello.elf", &mut mapper, &mut frame_allocator);

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

    serial_println!("System halted due to panic, entering infinite loop");

    sos::hlt_loop();
}
