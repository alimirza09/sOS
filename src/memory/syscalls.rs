use alloc::alloc::{alloc, dealloc, Layout};
use alloc::collections::BTreeMap;
use spin::Mutex;

static MMAP_ALLOCATIONS: Mutex<BTreeMap<u64, (usize, Layout)>> = Mutex::new(BTreeMap::new());

pub fn sys_mmap(addr: u64, length: u64, prot: u64) -> u64 {
    use crate::serial_println;
    serial_println!(
        "mmap: addr={:#x}, len={:#x}, prot={:#x}",
        addr,
        length,
        prot
    );

    if length == 0 {
        return u64::MAX;
    }

    let layout = match Layout::from_size_align(length as usize, 4096) {
        Ok(l) => l,
        Err(_) => {
            serial_println!("mmap: invalid layout");
            return u64::MAX;
        }
    };

    let ptr = unsafe { alloc(layout) };

    if ptr.is_null() {
        serial_println!("mmap: allocation failed");
        return u64::MAX;
    }

    let addr = ptr as u64;

    unsafe {
        core::ptr::write_bytes(ptr, 0, length as usize);
    }

    MMAP_ALLOCATIONS
        .lock()
        .insert(addr, (length as usize, layout));

    serial_println!("mmap: allocated {} bytes at {:#x}", length, addr);
    addr
}

pub fn sys_munmap(addr: u64, length: u64, _unused: u64) -> u64 {
    use crate::serial_println;

    serial_println!("munmap: addr={:#x}, len={:#x}", addr, length);

    if length == 0 {
        return 0;
    }

    let mut allocations = MMAP_ALLOCATIONS.lock();

    if let Some((alloc_size, layout)) = allocations.remove(&addr) {
        unsafe {
            dealloc(addr as *mut u8, layout);
        }
        serial_println!("munmap: freed {} bytes at {:#x}", alloc_size, addr);
        0
    } else {
        serial_println!("munmap: address not found in allocations");
        u64::MAX
    }
}

static CURRENT_BRK: Mutex<u64> = Mutex::new(0x4444_4444_0000);

pub fn init_brk(initial_brk: u64) {
    *CURRENT_BRK.lock() = initial_brk;
}

const HEAP_BASE: u64 = 0x4444_4444_0000;

pub fn sys_brk(addr: u64, _1: u64, _2: u64) -> u64 {
    use crate::serial_println;
    let mut current_brk = CURRENT_BRK.lock();

    if addr == 0 {
        serial_println!("brk: query current={:#x}", *current_brk);
        return *current_brk;
    }

    let requested = if addr < HEAP_BASE {
        match HEAP_BASE.checked_add(addr) {
            Some(val) => val,
            None => {
                serial_println!("brk: invalid requested address (overflow)");
                return u64::MAX;
            }
        }
    } else {
        addr
    };

    serial_println!(
        "brk: requested={:#x}, current={:#x}",
        requested,
        *current_brk
    );

    *current_brk = requested.max(*current_brk);
    *current_brk
}
