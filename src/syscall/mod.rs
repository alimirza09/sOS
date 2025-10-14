//! # Syscall Table & Handlers
//!
//! This module provides the syscall dispatch mechanism for SOS.  
//!
//! - Each syscall has a **numeric identifier** (`SYS_*` constants).  
//! - `SYSCALLS` is a table mapping syscall numbers → handler functions.  
//! - `syscall_identifier` performs dispatch and logging.  
//! - Stub implementations exist for `exit`.  
//!
//! ## Supported Syscalls
//!
//! | Number | Name       | Function        | Arguments (`a0`, `a1`, `a2`)                                      |
//! |--------|------------|-----------------|--------------------------------------------------------------------|
//! | 0      | `open`     | `sys_open`      | `a0=path_ptr`, `a1=flags`, `a2=mode`                              |
//! | 1      | `read`     | `sys_read`      | `a0=fd`, `a1=buf_ptr`, `a2=count`                                 |
//! | 2      | `write`    | `sys_write`     | `a0=fd`, `a1=buf_ptr`, `a2=count`                                 |
//! | 3      | `close`    | `sys_close`     | `a0=fd`, `a1=_`, `a2=_`                                           |
//! | 4      | `unlink`   | `sys_unlink`    | `a0=path_ptr`, `a1=_`, `a2=_`                                     |
//! | 5      | `mkdir`    | `sys_mkdir`     | `a0=path_ptr`, `a1=mode`, `a2=_`                                  |
//! | 6      | `rmdir`    | `sys_rmdir`     | `a0=path_ptr`, `a1=_`, `a2=_`                                     |
//! | 7      | `listdir`  | `sys_listdir`   | `a0=path_ptr`, `a1=buf_ptr`, `a2=size`                            |
//! | 8      | `exit`     | `sys_exit`      | `a0=exit_code`, `a1=_`, `a2=_`                                    |
//! | 9      | `mmap`     | `sys_mmap`      | `a0=addr`, `a1=size`, `a2=flags`                                  |
//! | 10     | `munmap`   | `sys_munmap`    | `a0=addr`, `a1=size`, `a2=_`                                      |
//! | 11     | `brk`      | `sys_brk`       | `a0=addr (0=query current)`, `a1=_`, `a2=_`                       |
//!
//! ## Notes
//! - Unrecognized syscall numbers return `u64::MAX`.
//! - All syscalls follow the same prototype:  
//!   `fn(u64, u64, u64) -> u64`

use crate::fs::syscalls::{
    sys_close, sys_listdir, sys_mkdir, sys_open, sys_read, sys_rmdir, sys_unlink, sys_write,
};
use crate::memory::syscalls::*;
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
pub const SYS_MMAP: u64 = 9;
pub const SYS_MUNMAP: u64 = 10;
pub const SYS_BRK: u64 = 11;

/// Global syscall dispatch table.  
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
    sys_mmap,
    sys_munmap,
    sys_brk,
];

/// Returns the syscall result as `u64`. Unknown syscalls return `u64::MAX`.
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
