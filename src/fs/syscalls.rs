use crate::fs::fat;
use crate::println;
use alloc::string::String;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

static FILE_OFFSET: AtomicUsize = AtomicUsize::new(0);
static FILE_SIZE: AtomicUsize = AtomicUsize::new(0);

lazy_static::lazy_static! {
    static ref LAST_FILENAME: Mutex<Option<String>> = Mutex::new(None);
    static ref READ_BUFFER: Mutex<[u8; 1024]> = Mutex::new([0u8; 1024]);
}

pub unsafe fn copy_in_cstr(ptr: u64) -> String {
    let mut buf = alloc::vec::Vec::new();
    let mut p = ptr as *const u8;
    loop {
        let c = ptr::read(p);
        if c == 0 {
            break;
        }
        buf.push(c);
        p = p.add(1);
    }
    String::from_utf8(buf).unwrap_or_default()
}

pub fn sys_open(filename_ptr: u64, write_flag: u64, _unused: u64) -> u64 {
    let filename = unsafe { copy_in_cstr(filename_ptr) };
    *LAST_FILENAME.lock() = Some(filename.clone());

    if write_flag != 0 {
        FILE_OFFSET.store(0, Ordering::SeqCst);
        FILE_SIZE.store(0, Ordering::SeqCst);
        3
    } else {
        if let Ok(size) = fat::file_size(&filename) {
            FILE_OFFSET.store(0, Ordering::SeqCst);
            FILE_SIZE.store(size, Ordering::SeqCst);
            3
        } else {
            u64::MAX
        }
    }
}

pub fn sys_write(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let len = count as usize;

    serial_println!("sys_write(fd={}, buf={:#x}, len={})", fd, buf_ptr, len);

    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, len) };

    match fd {
        1 => {
            unsafe {
                println!("{}", alloc::str::from_utf8_unchecked(buf));
            }
            count
        }
        2 => {
            if let Ok(s) = str::from_utf8(buf) {
                serial_println!("{}", s);
            } else {
                serial_println!("stderr non-utf8: {:02x?}", buf);
            }
            count
        }
        _ => {
            let filename = LAST_FILENAME.lock().clone().unwrap_or_default();
            if filename.is_empty() {
                serial_println!("sys_write: no filename set");
                return u64::MAX;
            }

            serial_println!("Writing {} bytes to file: {}", len, filename);

            match fat::write_file(&filename, buf) {
                Ok(()) => {
                    serial_println!("File write successful");
                    count
                }
                Err(e) => {
                    serial_println!("File write failed: {:?}", e);
                    u64::MAX
                }
            }
        }
    }
}

pub fn sys_read(_fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let filename = LAST_FILENAME.lock().clone().unwrap_or_default();

    let offset = FILE_OFFSET.load(Ordering::SeqCst);
    let size = FILE_SIZE.load(Ordering::SeqCst);

    if offset >= size {
        return 0;
    }

    let to_read = core::cmp::min(count as usize, size - offset);

    let mut temp_buf = alloc::vec::Vec::with_capacity(to_read);
    temp_buf.resize(to_read, 0);

    match fat::read_file_range(&filename, offset, &mut temp_buf[..]) {
        Ok(n) => {
            unsafe {
                ptr::copy_nonoverlapping(temp_buf.as_ptr(), buf_ptr as *mut u8, n);
            }
            FILE_OFFSET.store(offset + n, Ordering::SeqCst);
            n as u64
        }
        Err(_) => u64::MAX,
    }
}

use crate::serial_println;
use core::str;

pub fn sys_close(_fd: u64, _a1: u64, _a2: u64) -> u64 {
    0
}

pub fn sys_unlink(filename_ptr: u64, _a1: u64, _a2: u64) -> u64 {
    let filename = unsafe { copy_in_cstr(filename_ptr) };
    fat::remove_file(&filename).is_ok() as u64
}

pub fn sys_mkdir(path_ptr: u64, _a1: u64, _a2: u64) -> u64 {
    let path = unsafe { copy_in_cstr(path_ptr) };
    fat::create_dir(&path).is_ok() as u64
}

pub fn sys_rmdir(path_ptr: u64, _a1: u64, _a2: u64) -> u64 {
    let path = unsafe { copy_in_cstr(path_ptr) };
    fat::remove_dir(&path).is_ok() as u64
}

pub fn sys_listdir(path_ptr: u64, buf_ptr: u64, max: u64) -> u64 {
    let path = unsafe { copy_in_cstr(path_ptr) };
    match fat::list_dir(&path) {
        Ok(entries) => {
            let count = entries.len().min(max as usize);
            for (i, name) in entries.into_iter().take(count).enumerate() {
                unsafe {
                    let p = (buf_ptr as *mut u8).add(i * 256);
                    let bytes = name.as_bytes();
                    let len = bytes.len().min(255);
                    ptr::copy_nonoverlapping(bytes.as_ptr(), p, len);
                    *p.add(len) = 0;
                }
            }
            count as u64
        }
        Err(_) => u64::MAX,
    }
}
