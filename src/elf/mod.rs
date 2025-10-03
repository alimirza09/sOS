use alloc::vec::Vec;
use core::mem;
use x86_64::VirtAddr;
pub mod loader;
pub mod runner;

pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
pub const ELF_CLASS_64: u8 = 2;
pub const ELF_DATA_LSB: u8 = 1;
pub const ELF_VERSION_CURRENT: u8 = 1;
pub const ELF_TYPE_EXEC: u16 = 2;
pub const ELF_MACHINE_X86_64: u16 = 0x3e;

pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;
pub const PT_SHLIB: u32 = 5;
pub const PT_PHDR: u32 = 6;

pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

#[repr(C)]
pub struct ElfHeader {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

#[repr(C)]
pub struct ProgramHeader {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

pub struct ElfFile {
    pub data: Vec<u8>,
    pub header: ElfHeader,
    pub program_headers: Vec<ProgramHeader>,
}

impl ElfFile {
    pub fn from_data(data: Vec<u8>) -> Result<Self, ElfError> {
        if data.len() < mem::size_of::<ElfHeader>() {
            return Err(ElfError::InvalidFormat);
        }

        let header = unsafe { core::ptr::read(data.as_ptr() as *const ElfHeader) };

        if header.e_ident[0..4] != ELF_MAGIC {
            return Err(ElfError::InvalidMagic);
        }

        if header.e_ident[4] != ELF_CLASS_64 {
            return Err(ElfError::UnsupportedArchitecture);
        }

        if header.e_ident[5] != ELF_DATA_LSB {
            return Err(ElfError::UnsupportedEndianness);
        }

        if header.e_type != ELF_TYPE_EXEC {
            return Err(ElfError::NotExecutable);
        }

        if header.e_machine != ELF_MACHINE_X86_64 {
            return Err(ElfError::UnsupportedArchitecture);
        }

        let mut program_headers = Vec::new();
        let ph_offset = header.e_phoff as usize;

        if ph_offset + (header.e_phnum as usize * header.e_phentsize as usize) > data.len() {
            return Err(ElfError::InvalidFormat);
        }

        for i in 0..header.e_phnum {
            let offset = ph_offset + (i as usize * header.e_phentsize as usize);
            let ph = unsafe {
                core::ptr::read((data.as_ptr() as usize + offset) as *const ProgramHeader)
            };
            program_headers.push(ph);
        }

        Ok(ElfFile {
            data,
            header,
            program_headers,
        })
    }

    pub fn entry_point(&self) -> VirtAddr {
        VirtAddr::new(self.header.e_entry)
    }

    pub fn loadable_segments(&self) -> impl Iterator<Item = &ProgramHeader> {
        self.program_headers
            .iter()
            .filter(|ph| ph.p_type == PT_LOAD)
    }
}

#[derive(Debug)]
pub enum ElfError {
    InvalidMagic,
    InvalidFormat,
    UnsupportedArchitecture,
    UnsupportedEndianness,
    NotExecutable,
    LoadError,
    OutOfMemory,
    InvalidVirtualAddress,
}

impl core::fmt::Display for ElfError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            ElfError::InvalidMagic => write!(f, "Invalid ELF magic number"),
            ElfError::InvalidFormat => write!(f, "Invalid ELF format"),
            ElfError::UnsupportedArchitecture => write!(f, "Unsupported architecture"),
            ElfError::UnsupportedEndianness => write!(f, "Unsupported endianness"),
            ElfError::NotExecutable => write!(f, "Not an executable file"),
            ElfError::LoadError => write!(f, "Failed to load ELF file"),
            ElfError::OutOfMemory => write!(f, "Out of memory"),
            ElfError::InvalidVirtualAddress => write!(f, "Invalid virtual address"),
        }
    }
}
