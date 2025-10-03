use crate::syscall::{self, SYS_CLOSE, SYS_OPEN, SYS_READ};

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
