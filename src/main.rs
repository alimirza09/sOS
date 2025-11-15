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
    let (mut frame_allocator, mut mapper) = sos::init(boot_info);
    set_colors(Color::Green, Color::Black);
    println!("Welcome to sOS!");
    serial_println!("Welcome to sOS!");

    let drive_info = match sos::ata::identify_drive(true, AtaDevice::Slave) {
        Ok(info) => {
            serial_println!("Primary Slave found:");
            serial_println!("  Model: {}", info.model);
            serial_println!("  Capacity: {} GB", info.capacity_gb());
            Some(info)
        }
        Err(e) => {
            serial_println!("Primary Slave error: {:?}", e);
            None
        }
    };
    sos::fs::fat::mount_root_fs(
        sos::ata::AtaDevice::Slave,
        drive_info.unwrap().sectors as u32,
    );
    if let Ok(some) = sos::fs::fat::list_dir("") {
        serial_println!("Files: {:?}", some);
    }
    serial_println!("==================================");

    if let Some(gpu_dev) = sos::drivers::pci::find_virtio_gpu() {
        serial_println!("Initializing VirtIO-GPU");
        let mut gpu = sos::drivers::pci::VirtioGpu::new(gpu_dev);
        match gpu.init(&mut mapper, &mut frame_allocator) {
            Ok(()) => {
                serial_println!("VirtIO-GPU initialized successfully");
                *sos::GPU.lock() = Some(gpu);
            }
            Err(e) => {
                serial_println!("Failed to initialize VirtIO-GPU: {}", e);
            }
        }
    } else {
        serial_println!("No VirtIO-GPU device found");
    }

    *sos::MAPPER.lock() = Some(mapper);
    *sos::FRAME_ALLOCATOR.lock() = Some(frame_allocator);

    serial_println!("==================================");
    serial_println!("System initialized");
    serial_println!("==================================");

    sos::elf::runner::run_elf_in_kernel_mode("hello.elf");

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

    serial_println!("Panic message: {}", info.message());
    sos::hlt_loop();
}
