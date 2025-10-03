use alloc::string::{String, ToString};
use alloc::vec::Vec;
use embedded_sdmmc::{Mode, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use spin::Mutex;

use crate::fs::ata_block::SosAtaBlockDevice;

pub struct DummyTime;
impl TimeSource for DummyTime {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 54,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

pub static VOLUME_MANAGER: Mutex<Option<VolumeManager<SosAtaBlockDevice, DummyTime>>> =
    Mutex::new(None);

pub fn mount_root_fs(device: crate::drivers::ata::AtaDevice, block_count: u32) {
    let dev = SosAtaBlockDevice {
        primary: true,
        device,
        block_count,
    };
    let manager = VolumeManager::new(dev, DummyTime);
    *VOLUME_MANAGER.lock() = Some(manager);
}

pub fn file_size(path: &str) -> Result<usize, &'static str> {
    let components = split_path(path);

    if components.len() != 1 {
        return Err("Only root directory files supported currently");
    }

    let file_name = components[0];

    let mut guard = VOLUME_MANAGER.lock();
    let manager = guard.as_mut().ok_or("No volume manager")?;
    let mut volume = manager
        .open_volume(VolumeIdx(0))
        .map_err(|_| "open_volume failed")?;

    let mut root_dir = volume.open_root_dir().map_err(|_| "open_root_dir failed")?;
    let file = root_dir
        .open_file_in_dir(file_name, Mode::ReadOnly)
        .map_err(|_| "open_file failed")?;

    Ok(file.length() as usize)
}

pub fn read_file_range(path: &str, offset: usize, buf: &mut [u8]) -> Result<usize, &'static str> {
    let components = split_path(path);

    if components.len() != 1 {
        return Err("Only root directory files supported currently");
    }

    let file_name = components[0];

    let mut guard = VOLUME_MANAGER.lock();
    let manager = guard.as_mut().ok_or("No volume manager")?;
    let mut volume = manager
        .open_volume(VolumeIdx(0))
        .map_err(|_| "open_volume failed")?;

    let mut root_dir = volume.open_root_dir().map_err(|_| "open_root_dir failed")?;
    let mut file = root_dir
        .open_file_in_dir(file_name, Mode::ReadOnly)
        .map_err(|_| "open_file failed")?;

    if offset > 0 {
        let mut skip_buf = [0u8; 512];
        let mut skipped = 0;
        while skipped < offset {
            let to_skip = core::cmp::min(skip_buf.len(), offset - skipped);
            let n = file
                .read(&mut skip_buf[..to_skip])
                .map_err(|_| "file.read failed")?;
            if n == 0 {
                return Ok(0); // EOF
            }
            skipped += n;
        }
    }

    let n = file.read(buf).map_err(|_| "file.read failed")?;
    Ok(n)
}

fn split_path(path: &str) -> Vec<&str> {
    path.split('/').filter(|p| !p.is_empty()).collect()
}

pub fn write_file(path: &str, data: &[u8]) -> Result<(), &'static str> {
    let components = split_path(path);

    if components.len() != 1 {
        return Err("Only root directory files supported currently");
    }

    let file_name = components[0];

    let mut guard = VOLUME_MANAGER.lock();
    let manager = guard.as_mut().ok_or("No volume manager")?;
    let mut volume = manager
        .open_volume(VolumeIdx(0))
        .map_err(|_| "open_volume failed")?;

    let mut root_dir = volume.open_root_dir().map_err(|_| "open_root_dir failed")?;
    let mut file = root_dir
        .open_file_in_dir(file_name, Mode::ReadWriteCreateOrTruncate)
        .map_err(|_| "open_file failed")?;
    file.write(data).map_err(|_| "file.write failed")?;
    Ok(())
}

pub fn read_file(path: &str, buf: &mut [u8]) -> Result<usize, &'static str> {
    let components = split_path(path);

    if components.len() != 1 {
        return Err("Only root directory files supported currently");
    }

    let file_name = components[0];

    let mut guard = VOLUME_MANAGER.lock();
    let manager = guard.as_mut().ok_or("No volume manager")?;
    let mut volume = manager
        .open_volume(VolumeIdx(0))
        .map_err(|_| "open_volume failed")?;

    let mut root_dir = volume.open_root_dir().map_err(|_| "open_root_dir failed")?;
    let mut file = root_dir
        .open_file_in_dir(file_name, Mode::ReadOnly)
        .map_err(|_| "open_file failed")?;
    let n = file.read(buf).map_err(|_| "file.read failed")?;
    Ok(n)
}

pub fn remove_file(path: &str) -> Result<(), &'static str> {
    let components = split_path(path);

    if components.len() != 1 {
        return Err("Only root directory files supported currently");
    }

    let file_name = components[0];

    let mut guard = VOLUME_MANAGER.lock();
    let manager = guard.as_mut().ok_or("No volume manager")?;
    let mut volume = manager
        .open_volume(VolumeIdx(0))
        .map_err(|_| "open_volume failed")?;

    let mut root_dir = volume.open_root_dir().map_err(|_| "open_root_dir failed")?;
    root_dir
        .delete_file_in_dir(file_name)
        .map_err(|_| "delete_file failed")?;
    Ok(())
}

pub fn create_dir(path: &str) -> Result<(), &'static str> {
    let components = split_path(path);

    if components.len() != 1 {
        return Err("Only root directory creation supported currently");
    }

    let dir_name = components[0];

    let mut guard = VOLUME_MANAGER.lock();
    let manager = guard.as_mut().ok_or("No volume manager")?;
    let mut volume = manager
        .open_volume(VolumeIdx(0))
        .map_err(|_| "open_volume failed")?;

    let mut root_dir = volume.open_root_dir().map_err(|_| "open_root_dir failed")?;
    root_dir
        .make_dir_in_dir(dir_name)
        .map_err(|_| "make_dir_in_dir failed")?;
    Ok(())
}

pub fn remove_dir(path: &str) -> Result<(), &'static str> {
    let components = split_path(path);

    if components.len() != 1 {
        return Err("Only root directory removal supported currently");
    }

    let dir_name = components[0];

    let mut guard = VOLUME_MANAGER.lock();
    let manager = guard.as_mut().ok_or("No volume manager")?;
    let mut volume = manager
        .open_volume(VolumeIdx(0))
        .map_err(|_| "open_volume failed")?;

    let mut root_dir = volume.open_root_dir().map_err(|_| "open_root_dir failed")?;

    root_dir
        .delete_file_in_dir(dir_name)
        .map_err(|_| "Directory removal failed - method may not exist or directory not empty")?;
    Ok(())
}

pub fn list_dir(path: &str) -> Result<Vec<String>, &'static str> {
    let components = split_path(path);

    if !components.is_empty() {
        return Err("Only root directory listing supported currently");
    }

    let mut guard = VOLUME_MANAGER.lock();
    let manager = guard.as_mut().ok_or("No volume manager")?;
    let mut volume = manager
        .open_volume(VolumeIdx(0))
        .map_err(|_| "open_volume failed")?;

    let mut root_dir = volume.open_root_dir().map_err(|_| "open_root_dir failed")?;
    let mut names = Vec::new();
    root_dir
        .iterate_dir(|entry| {
            names.push(entry.name.to_string());
        })
        .map_err(|_| "iterate_dir failed")?;
    Ok(names)
}
