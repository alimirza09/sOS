use crate::fs::fat;
use crate::println;
use alloc::string::String;
use alloc::vec::Vec;
use core::ptr;
use spin::Mutex;

#[derive(Clone)]
struct OpenFile {
    filename: String,
    offset: usize,
    size: usize,
    writable: bool,
}

struct FdTable {
    files: Vec<Option<OpenFile>>,
}

impl FdTable {
    fn new() -> Self {
        Self {
            files: alloc::vec![None, None, None],
        }
    }

    fn alloc_fd(&mut self, file: OpenFile) -> Option<usize> {
        for (i, slot) in self.files.iter_mut().enumerate().skip(3) {
            if slot.is_none() {
                *slot = Some(file);
                return Some(i);
            }
        }
        self.files.push(Some(file));
        Some(self.files.len() - 1)
    }

    fn get(&self, fd: usize) -> Option<&OpenFile> {
        self.files.get(fd).and_then(|f| f.as_ref())
    }

    fn get_mut(&mut self, fd: usize) -> Option<&mut OpenFile> {
        self.files.get_mut(fd).and_then(|f| f.as_mut())
    }

    fn close(&mut self, fd: usize) -> bool {
        if fd < 3 {
            return false;
        }

        if let Some(slot) = self.files.get_mut(fd) {
            if slot.is_some() {
                *slot = None;
                return true;
            }
        }
        false
    }
}

lazy_static::lazy_static! {
    static ref FD_TABLE: Mutex<FdTable> = Mutex::new(FdTable::new());
}

//    pub unsafe fn copy_in_cstr(ptr: u64) -> String {
//        let mut buf = alloc::vec::Vec::new();
//        let mut p = ptr as *const u8;
//        loop {
//            let c = ptr::read(p);
//            if c == 0 {
//                break;
//            }
//            buf.push(c);
//            p = p.add(1);
//        }
//        String::from_utf8(buf).unwrap_or_default()
//    }

pub unsafe fn copy_in_cstr(ptr: u64) -> String {
    let mut buf = Vec::new();
    let mut p = ptr as *const u8;
    for _ in 0..256 {
        let c = ptr::read(p);
        if c == 0 {
            break;
        }
        buf.push(c);
        p = p.add(1);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

pub fn sys_open(filename_ptr: u64, flags: u64, _mode: u64) -> u64 {
    serial_println!(
        "sys_open: filename_ptr={:#x}, flags={}",
        filename_ptr,
        flags
    );
    let filename = unsafe { copy_in_cstr(filename_ptr) };
    serial_println!("sys_open: filename='{}'", filename);

    let writable = flags & 1 != 0;
    let readable = flags == 0 || flags & 2 != 0;

    let size = if readable {
        match fat::file_size(&filename) {
            Ok(s) => s,
            Err(_) if !writable => {
                return u64::MAX;
            }
            Err(_) => 0,
        }
    } else {
        0
    };

    let file = OpenFile {
        filename,
        offset: 0,
        size,
        writable,
    };

    let mut table = FD_TABLE.lock();
    match table.alloc_fd(file) {
        Some(fd) => {
            serial_println!("sys_open: Allocated fd: {}", fd);
            fd as u64
        }
        None => u64::MAX,
    }
}

pub fn sys_read(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let fd = fd as usize;
    let table = FD_TABLE.lock();

    let file = match table.get(fd) {
        Some(f) => f,
        None => return u64::MAX,
    };

    if file.offset >= file.size {
        return 0;
    }

    let to_read = core::cmp::min(count as usize, file.size - file.offset);
    let filename = file.filename.clone();
    let offset = file.offset;

    drop(table);

    let mut temp_buf = alloc::vec::Vec::with_capacity(to_read);
    temp_buf.resize(to_read, 0);

    match fat::read_file_range(&filename, offset, &mut temp_buf[..]) {
        Ok(n) => {
            unsafe {
                ptr::copy_nonoverlapping(temp_buf.as_ptr(), buf_ptr as *mut u8, n);
            }

            let mut table = FD_TABLE.lock();
            if let Some(file) = table.get_mut(fd) {
                file.offset += n;
            }

            n as u64
        }
        Err(_) => u64::MAX,
    }
}

use crate::serial_println;
use core::str;

pub fn sys_write(fd: u64, buf_ptr: u64, count: u64) -> u64 {
    let len = count as usize;
    let buf = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, len) };
    serial_println!("sys_write being executed");

    match fd {
        0 => {
            serial_println!("stdin???");
            count
        }
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
            let fd = fd as usize;
            let table = FD_TABLE.lock();

            serial_println!("sys_write default case being executed");

            let file = match table.get(fd) {
                Some(f) => f,
                None => {
                    serial_println!("sys_write: invalid fd {}", fd);
                    return u64::MAX;
                }
            };

            if !file.writable {
                serial_println!("sys_write: fd {} not writable", fd);
                return u64::MAX;
            }

            let filename = file.filename.clone();
            drop(table);

            serial_println!("Writing {} bytes to file: {}", len, filename);

            match fat::write_file(&filename, buf) {
                Ok(()) => {
                    serial_println!("File write successful");

                    let mut table = FD_TABLE.lock();
                    if let Some(file) = table.get_mut(fd as usize) {
                        file.size = len;
                        file.offset = len;
                    }

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

pub fn sys_close(fd: u64, _a1: u64, _a2: u64) -> u64 {
    let fd = fd as usize;

    if fd < 3 {
        return 0;
    }

    let mut table = FD_TABLE.lock();
    if table.close(fd) {
        0
    } else {
        u64::MAX
    }
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
