use crate::fs::syscalls::{
    sys_close, sys_listdir, sys_mkdir, sys_open, sys_read, sys_rmdir, sys_unlink, sys_write,
};
use crate::serial_println;

pub const SYS_OPEN: u64 = 0;
pub const SYS_READ: u64 = 1;
pub const SYS_WRITE: u64 = 2;
pub const SYS_CLOSE: u64 = 3;
pub const SYS_UNLINK: u64 = 4;
pub const SYS_MKDIR: u64 = 5;
pub const SYS_RMDIR: u64 = 6;
pub const SYS_LISTDIR: u64 = 7;
pub const SYS_EXIT: u64 = 8;
pub const SYS_BRK: u64 = 9;
pub const SYS_MMAP: u64 = 10;

pub const SYSCALLS: &[fn(u64, u64, u64) -> u64] = &[
    sys_open,
    sys_read,
    sys_write,
    sys_close,
    sys_unlink,
    sys_mkdir,
    sys_rmdir,
    sys_listdir,
    sys_exit,
    sys_brk,
    sys_mmap,
];

pub fn syscall_identifier(num: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    let idx = num as usize;
    if idx < SYSCALLS.len() {
        serial_println!("syscall: {} ({}, {}, {})", num, a0, a1, a2);
        SYSCALLS[idx](a0, a1, a2)
    } else {
        serial_println!("syscall: unknown syscall number {}", num);
        u64::MAX
    }
}

pub fn sys_exit(code: u64, _a1: u64, _a2: u64) -> u64 {
    use crate::serial_println;
    serial_println!("Process exit with code: {}", code);

    // TODO: cleanup process structures

    // NOTE: TEMPORARY
    crate::hlt_loop();
}

pub fn sys_brk(addr: u64, _a1: u64, _a2: u64) -> u64 {
    crate::serial_println!("brk() called with addr: {:#x}", addr);
    addr
}

pub fn sys_mmap(addr: u64, length: u64, prot: u64) -> u64 {
    crate::serial_println!(
        "mmap() called: addr={:#x}, len={:#x}, prot={:#x}",
        addr,
        length,
        prot
    );
    addr
}
