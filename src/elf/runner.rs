use crate::elf::*;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::Translate;
use x86_64::structures::paging::{FrameAllocator, Page, PageTableFlags, Size4KiB};
use x86_64::structures::paging::{Mapper, OffsetPageTable, PageSize};
use x86_64::VirtAddr;

const PAGE_SIZE: usize = 4096;
const STACK_BASE: u64 = 0x0000_7FFF_F000_0000;
const USER_BASE: u64 = 0x400000;

fn map_pages_at(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    start: VirtAddr,
    size: usize,
    mut flags: PageTableFlags,
) -> Result<(), &'static str> {
    flags |= PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;

    let start_page = Page::containing_address(start);
    let end_addr = start + (size as u64) - 1u64;
    let end_page = Page::containing_address(end_addr);

    let mut current_page = start_page;
    loop {
        if mapper
            .translate_addr(current_page.start_address())
            .is_some()
        {
            return Err("virtual page already mapped (virtual address conflict)");
        }

        let frame = frame_allocator.allocate_frame().ok_or("No free frames")?;

        let res = unsafe { mapper.map_to(current_page, frame, flags, frame_allocator) };
        match res {
            Ok(flush) => {
                flush.flush();
            }
            Err(e) => {
                return Err(match e {
                    MapToError::FrameAllocationFailed => "map_to error: FrameAllocationFailed",
                    MapToError::ParentEntryHugePage => "map_to error: ParentEntryHugePage",
                    _ => "map_to error: unknown",
                });
            }
        }

        if current_page == end_page {
            break;
        }

        current_page = Page::containing_address(current_page.start_address() + Size4KiB::SIZE);
    }

    use x86_64::instructions::tlb;
    tlb::flush_all();

    Ok(())
}

fn alloc_stack(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    num_pages: usize,
) -> Result<VirtAddr, &'static str> {
    let size = (num_pages as u64) * Size4KiB::SIZE;
    let start = VirtAddr::new(STACK_BASE);

    let mut flags = PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
    flags |= PageTableFlags::USER_ACCESSIBLE;

    map_pages_at(mapper, frame_allocator, start, size as usize, flags)?;

    Ok(start + size)
}

pub fn run_elf_in_kernel_mode(file: &str) {
    use crate::elf::loader;
    use crate::elf::ElfFile;
    use crate::paging::BootInfoFrameAllocator;
    use crate::serial_println;
    use crate::{FRAME_ALLOCATOR, MAPPER};
    use core::ptr;
    use x86_64::structures::paging::PageTableFlags;
    use x86_64::VirtAddr;

    let mut mapper_opt = {
        let mut g = MAPPER.lock();
        g.take()
    };
    let mut frame_alloc_opt = {
        let mut g = FRAME_ALLOCATOR.lock();
        g.take()
    };

    fn restore_globals(
        mapper_opt: Option<OffsetPageTable<'static>>,
        frame_alloc_opt: Option<BootInfoFrameAllocator>,
    ) {
        if let Some(mapper) = mapper_opt {
            let mut g = MAPPER.lock();
            *g = Some(mapper);
        }
        if let Some(fa) = frame_alloc_opt {
            let mut g = FRAME_ALLOCATOR.lock();
            *g = Some(fa);
        }
    }

    if mapper_opt.is_none() {
        serial_println!("run_elf_in_kernel_mode: MAPPER is None");
        restore_globals(mapper_opt, frame_alloc_opt);
        return;
    }
    if frame_alloc_opt.is_none() {
        serial_println!("run_elf_in_kernel_mode: FRAME_ALLOCATOR is None");
        restore_globals(mapper_opt, frame_alloc_opt);
        return;
    }

    let mut mapper = mapper_opt.take().unwrap();
    let mut frame_allocator = frame_alloc_opt.take().unwrap();

    let contents = match loader::extract_elf_exec(file) {
        Some(c) => c,
        None => {
            serial_println!("Failed to read ELF file: {}", file);
            restore_globals(Some(mapper), Some(frame_allocator));
            return;
        }
    };

    let elf = match ElfFile::from_data(contents.clone()) {
        Ok(e) => e,
        Err(e) => {
            serial_println!("ELF parse error: {}", e);
            restore_globals(Some(mapper), Some(frame_allocator));
            return;
        }
    };

    let original_base = elf
        .loadable_segments()
        .next()
        .map(|ph| ph.p_vaddr)
        .unwrap_or(0);
    let relocation_offset = USER_BASE.wrapping_sub(original_base);

    serial_println!("ELF parsed: entry={:?}", elf.entry_point());
    serial_println!(
        "Relocating from {:#x} to {:#x} (offset: {:#x})",
        original_base,
        USER_BASE,
        relocation_offset
    );

    for ph in elf.loadable_segments() {
        let file_size = ph.p_filesz as usize;
        let mem_size = ph.p_memsz as usize;
        if mem_size == 0 {
            continue;
        }

        let relocated_vaddr = ph.p_vaddr.wrapping_add(relocation_offset);

        let seg_start_page_addr = (relocated_vaddr as usize) & !(PAGE_SIZE - 1);
        let seg_end_addr = (relocated_vaddr as usize).saturating_add(mem_size);
        let seg_end_page_addr = (seg_end_addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let map_size = seg_end_page_addr.saturating_sub(seg_start_page_addr);
        let map_vaddr = VirtAddr::new(seg_start_page_addr as u64);

        let mut flags = PageTableFlags::WRITABLE;
        if (ph.p_flags & 0x1) == 0 {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        serial_println!(
            "Mapping segment: vaddr={:#x} (orig {:#x}) filesz={} memsz={} map_size={}",
            relocated_vaddr,
            ph.p_vaddr,
            file_size,
            mem_size,
            map_size
        );

        if let Err(e) = map_pages_at(
            &mut mapper,
            &mut frame_allocator,
            map_vaddr,
            map_size,
            flags,
        ) {
            serial_println!("Failed to map segment at {:#x}: {}", relocated_vaddr, e);
            restore_globals(Some(mapper), Some(frame_allocator));
            return;
        }

        unsafe {
            let dest = VirtAddr::new(relocated_vaddr).as_mut_ptr::<u8>();
            if file_size > 0 {
                let src_ptr = contents.as_ptr().add(ph.p_offset as usize);
                ptr::copy_nonoverlapping(src_ptr, dest, file_size);
            }
            if mem_size > file_size {
                let bss_ptr = dest.add(file_size);
                let bss_len = mem_size - file_size;
                ptr::write_bytes(bss_ptr, 0, bss_len);
            }
        }

        serial_println!(
            "Mapped seg vaddr={:#x} filesz={} memsz={} mapped_bytes={}",
            relocated_vaddr,
            file_size,
            mem_size,
            map_size
        );
    }

    if elf.header.e_type == ELF_TYPE_DYN {
        serial_println!("Processing relocations for PIE binary...");

        let mut rela_offset = 0u64;
        let mut rela_size = 0u64;

        for ph in elf.program_headers.iter() {
            if ph.p_type == PT_DYNAMIC {
                let dynamic_vaddr = ph.p_vaddr.wrapping_add(relocation_offset);
                let dynamic_count = ph.p_memsz / 16;

                unsafe {
                    let dynamic_ptr = dynamic_vaddr as *const u64;
                    for i in 0..dynamic_count {
                        let tag = *dynamic_ptr.add((i * 2) as usize);
                        let val = *dynamic_ptr.add((i * 2 + 1) as usize);

                        if tag == DT_NULL {
                            break;
                        }
                        if tag == DT_RELA {
                            rela_offset = val;
                        }
                        if tag == DT_RELASZ {
                            rela_size = val;
                        }
                    }
                }
                break;
            }
        }

        if rela_offset > 0 && rela_size > 0 {
            let rela_vaddr = rela_offset.wrapping_add(relocation_offset);
            let rela_count = rela_size / 24;

            serial_println!("Found {} relocations at {:#x}", rela_count, rela_vaddr);

            unsafe {
                let rela_ptr = rela_vaddr as *const u64;
                for i in 0..rela_count {
                    let r_offset = *rela_ptr.add((i * 3) as usize);
                    let r_info = *rela_ptr.add((i * 3 + 1) as usize);
                    let r_addend = *rela_ptr.add((i * 3 + 2) as usize) as i64;

                    let r_type = (r_info & 0xFFFFFFFF) as u32;

                    if r_type == R_X86_64_RELATIVE {
                        let target_addr = r_offset.wrapping_add(relocation_offset);
                        let value = (relocation_offset as i64 + r_addend) as u64;

                        *(target_addr as *mut u64) = value;
                    }
                }
            }

            serial_println!("Relocations applied successfully!");
        } else {
            serial_println!("No relocations found");
        }
    }

    let stack_pages = 16usize;
    let stack_top = match alloc_stack(&mut mapper, &mut frame_allocator, stack_pages) {
        Ok(v) => v,
        Err(e) => {
            serial_println!("Failed to allocate stack: {}", e);
            restore_globals(Some(mapper), Some(frame_allocator));
            return;
        }
    };

    let entry_addr = elf.entry_point().as_u64().wrapping_add(relocation_offset);

    serial_println!(
        "Preparing to jump to entry: {:#x} (orig {:#x}) with stack top {:#x}",
        entry_addr,
        elf.entry_point().as_u64(),
        stack_top.as_u64()
    );

    {
        let mut g = MAPPER.lock();
        *g = Some(mapper);
    }
    {
        let mut g = FRAME_ALLOCATOR.lock();
        *g = Some(frame_allocator);
    }

    unsafe {
        let entry_fn: usize = entry_addr as usize;

        let user_cs = crate::gdt::user_code_selector().0 as u64;
        let user_ss = crate::gdt::user_data_selector().0 as u64;

        let mut stack_ptr = stack_top.as_u64();
        stack_ptr -= 8;

        core::arch::asm!(
            "push {ss}",
            "push {stack}",
            "push 0x202",
            "push {cs}",
            "push {entry}",
            "iretq",

            ss = in(reg) user_ss,
            cs = in(reg) user_cs,
            stack = in(reg) stack_ptr,
            entry = in(reg) entry_fn,
            options(noreturn)
        );
    }
}
