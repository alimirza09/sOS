use core::ptr;
use x86_64::structures::paging::{FrameAllocator, Page, PageTableFlags, Size4KiB};
use x86_64::structures::paging::{Mapper, OffsetPageTable, PageSize};
use x86_64::VirtAddr;

use crate::elf::ElfFile;

const PAGE_SIZE: usize = 4096;
const STACK_BASE: u64 = 0xFFFF_FF80_0000_0000;

fn map_pages_at(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    start: VirtAddr,
    size: usize,
    mut flags: PageTableFlags,
) -> Result<(), &'static str> {
    flags |= PageTableFlags::PRESENT;

    let start_page = Page::containing_address(start);
    let end_addr = start + (size as u64) - 1u64;
    let end_page = Page::containing_address(end_addr);

    let mut current_page = start_page;
    loop {
        let frame = frame_allocator.allocate_frame().ok_or("No free frames")?;

        unsafe {
            mapper
                .map_to(current_page, frame, flags, frame_allocator)
                .map_err(|_| "map_to failed")?
                .flush();
        }

        if current_page == end_page {
            break;
        }

        current_page = Page::containing_address(current_page.start_address() + Size4KiB::SIZE);
    }

    Ok(())
}

fn alloc_stack(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    num_pages: usize,
) -> Result<VirtAddr, &'static str> {
    let size = (num_pages as u64) * Size4KiB::SIZE;
    let start = VirtAddr::new(STACK_BASE);

    let flags = PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
    map_pages_at(mapper, frame_allocator, start, size as usize, flags)?;

    Ok(start + size)
}

pub fn run_elf_in_kernel_mode(
    file: &str,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let contents = match crate::elf::loader::extract_elf_exec(file) {
        Some(c) => c,
        None => {
            crate::serial_println!("Failed to read ELF file: {}", file);
            return;
        }
    };

    let elf = match ElfFile::from_data(contents.clone()) {
        Ok(e) => e,
        Err(e) => {
            crate::serial_println!("ELF parse error: {}", e);
            return;
        }
    };

    crate::serial_println!("ELF parsed: entry={:?}", elf.entry_point());

    for ph in elf.loadable_segments() {
        let file_size = ph.p_filesz as usize;
        let mem_size = ph.p_memsz as usize;
        let file_offset = ph.p_offset as usize;

        if mem_size == 0 {
            continue;
        }

        let seg_start_page_addr = (ph.p_vaddr as usize) & !(PAGE_SIZE - 1);
        let seg_end_addr = (ph.p_vaddr as usize).saturating_add(mem_size);
        let seg_end_page_addr = (seg_end_addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let map_size = seg_end_page_addr.saturating_sub(seg_start_page_addr);
        let map_vaddr = VirtAddr::new(seg_start_page_addr as u64);

        let mut flags = PageTableFlags::empty();
        flags |= PageTableFlags::WRITABLE;

        if (ph.p_flags & 0x1) == 0 {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        if let Err(e) = map_pages_at(mapper, frame_allocator, map_vaddr, map_size, flags) {
            crate::serial_println!("Failed to map segment at {:#x}: {}", ph.p_vaddr, e);
            return;
        }

        unsafe {
            let dest = VirtAddr::new(ph.p_vaddr).as_mut_ptr::<u8>();
            if file_size > 0 {
                let src_ptr = contents.as_ptr().add(file_offset);
                ptr::copy_nonoverlapping(src_ptr, dest, file_size);
            }
            if mem_size > file_size {
                let bss_ptr = dest.add(file_size);
                let bss_len = mem_size - file_size;
                ptr::write_bytes(bss_ptr, 0, bss_len);
            }
        }

        crate::serial_println!(
            "Mapped seg vaddr={:#x} filesz={} memsz={} mapped_bytes={}",
            ph.p_vaddr,
            file_size,
            mem_size,
            map_size
        );
    }

    let stack_pages = 16usize;
    let stack_top = match alloc_stack(mapper, frame_allocator, stack_pages) {
        Ok(v) => v,
        Err(e) => {
            crate::serial_println!("Failed to allocate stack: {}", e);
            return;
        }
    };

    let entry_addr = elf.entry_point().as_u64();

    crate::serial_println!(
        "Jumping to entry: {:#x} with stack top {:#x}",
        entry_addr,
        stack_top.as_u64()
    );

    let entry = elf.entry_point().as_u64() as usize;
    let base_vaddr = elf
        .loadable_segments()
        .next()
        .map(|ph| ph.p_vaddr as usize)
        .unwrap_or(0);
    let entry_offset = if entry >= base_vaddr {
        entry - base_vaddr
    } else {
        0
    };
    crate::serial_println!("entry bytes:");
    for i in 0..32 {
        let b = contents.get(entry_offset + i).copied().unwrap_or(0);
        crate::serial_print!("{}", alloc::format!("{:02x} ", b));
    }
    crate::serial_println!("");

    unsafe {
        let entry_fn: usize = entry_addr as usize;
        core::arch::asm!(
            "mov rsp, {0}",
            "xor rbp, rbp",
            "jmp {1}",
            in(reg) stack_top.as_u64(),
            in(reg) entry_fn,
            options(noreturn)
        );
    }
}
