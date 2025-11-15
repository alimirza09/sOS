use crate::syscall::{self, SYS_CLOSE, SYS_OPEN, SYS_READ};

use alloc::vec::Vec;

pub fn extract_elf_exec(file: &str) -> Option<Vec<u8>> {
    let mut path: Vec<u8> = file.as_bytes().to_vec();
    if path.is_empty() || *path.last().unwrap() != 0 {
        path.push(0);
    }
    let fd = syscall::syscall_identifier(SYS_OPEN, path.as_ptr() as u64, 0, 0) as i64;
    if fd < 0 {
        crate::serial_println!("extract_elf_exec: open failed");
        return None;
    }

    crate::serial_println!("extract_elf_exec: opened fd={}", fd);

    let mut out = Vec::new();
    let mut buf = [0u8; 1024];
    let mut total_read = 0;

    loop {
        let r = syscall::syscall_identifier(
            SYS_READ,
            fd as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        ) as i64;

        if r < 0 {
            crate::serial_println!("extract_elf_exec: read failed with {}", r);
            let _ = syscall::syscall_identifier(SYS_CLOSE, fd as u64, 0, 0);
            return None;
        }
        if r == 0 {
            break;
        }
        let n = r as usize;
        total_read += n;
        out.extend_from_slice(&buf[..n]);
    }

    crate::serial_println!("extract_elf_exec: read {} bytes total", total_read);
    let _ = syscall::syscall_identifier(SYS_CLOSE, fd as u64, 0, 0);
    Some(out)
}
