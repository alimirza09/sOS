#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

pub mod arch;
pub mod drivers;
pub mod elf;
pub mod fs;
pub mod memory;
pub mod sched;
pub mod sync;
pub mod syscall;
pub mod task;

pub use arch::x86_64::{gdt, interrupts, smp, timer};
pub use drivers::{ata, serial, sshell, vga_buffer};
pub use memory::{allocator, paging};
pub use sched::{context, processor, rr, thread_pool};
pub use sync::interrupt;
use x86_64::structures::paging::OffsetPageTable;

use crate::memory::BootInfoFrameAllocator;

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

use bootloader::BootInfo;
pub fn init(boot_info: &'static BootInfo) -> (BootInfoFrameAllocator, OffsetPageTable<'static>) {
    use x86_64::VirtAddr;

    arch::x86_64::gdt::init();
    arch::x86_64::interrupts::init_idt();
    unsafe { arch::x86_64::interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
    memory::syscalls::init_brk(allocator::HEAP_START as u64);
    enable_fpu_and_sse();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    let mut mapper = unsafe { paging::init(phys_mem_offset, &mut frame_allocator) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("Heap initialization failed");

    (frame_allocator, mapper)
}

use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

pub fn enable_fpu_and_sse() {
    let mut cr0 = Cr0::read();
    cr0.remove(Cr0Flags::EMULATE_COPROCESSOR);
    cr0.remove(Cr0Flags::TASK_SWITCHED);
    unsafe { Cr0::write(cr0) };

    let mut cr4 = Cr4::read();
    cr4.insert(Cr4Flags::OSFXSR);
    cr4.insert(Cr4Flags::OSXMMEXCPT_ENABLE);
    unsafe { Cr4::write(cr4) };

    unsafe {
        core::arch::asm!("fninit", options(nostack, nomem));
    }

    unsafe {
        core::arch::asm!("pxor xmm0, xmm0", options(nostack, nomem));
    }
}
