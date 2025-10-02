use crate::{
    elf::ElfFile,
    syscall::{self, SYS_CLOSE, SYS_OPEN, SYS_READ},
};

use alloc::vec::Vec;

pub fn extract_elf_exec(file: &str) -> Option<Vec<u8>> {
    let mut path: Vec<u8> = file.as_bytes().to_vec();
    if path.is_empty() || *path.last().unwrap() != 0 {
        path.push(0);
    }

    let fd = syscall::syscall_identifier(SYS_OPEN, path.as_ptr() as u64, 0, 0) as i64;
    if fd < 0 {
        return None;
    }

    let mut out = Vec::new();
    let mut buf = [0u8; 1024];

    loop {
        let r = syscall::syscall_identifier(
            SYS_READ,
            fd as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        ) as i64;

        if r < 0 {
            let _ = syscall::syscall_identifier(SYS_CLOSE, fd as u64, 0, 0);
            return None;
        }

        if r == 0 {
            break;
        }

        let n = r as usize;
        out.extend_from_slice(&buf[..n]);
    }

    let _ = syscall::syscall_identifier(SYS_CLOSE, fd as u64, 0, 0);

    Some(out)
}

pub fn run_elf_exec(file: &str) {
    let contents = match extract_elf_exec(file) {
        Some(data) => data,
        None => {
            crate::serial_println!("Failed to read ELF file: {}", file);
            return;
        }
    };

    match ElfFile::from_data(contents) {
        Ok(elf) => {
            crate::serial_println!("ELF parsed successfully!");
            crate::serial_println!("Entry point: {:?}", elf.entry_point());

            for ph in elf.loadable_segments() {
                crate::serial_println!(
                    "Loadable segment: vaddr={:#x}, filesz={}, memsz={}, flags={:#x}",
                    ph.p_vaddr,
                    ph.p_filesz,
                    ph.p_memsz,
                    ph.p_flags
                );
                // TODO: map pages + copy data into memory here
            }

            // TODO: jump to elf.entry_point() to execute
        }
        Err(err) => {
            crate::serial_println!("ELF parsing failed: {}", err);
        }
    }
}
