use crate::drivers::ata::{read_sectors, write_sectors, AtaDevice, AtaError};
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx};
use spin::Mutex;

static SECTOR_BUFFER: Mutex<[u8; 131072]> = Mutex::new([0u8; 131072]);

pub struct SosAtaBlockDevice {
    pub primary: bool,
    pub device: AtaDevice,
    pub block_count: u32,
}

impl BlockDevice for SosAtaBlockDevice {
    type Error = AtaError;

    fn read(
        &self,
        blocks: &mut [Block],
        start_block_idx: BlockIdx,
        _reason: &str,
    ) -> Result<(), Self::Error> {
        let total_blocks = blocks.len();
        let mut blocks_read = 0;
        let mut buffer = SECTOR_BUFFER.lock();

        while blocks_read < total_blocks {
            let blocks_remaining = total_blocks - blocks_read;
            let chunk_size = blocks_remaining.min(256);
            let chunk_bytes = chunk_size * 512;

            let lba = start_block_idx.0 as u64 + blocks_read as u64;

            read_sectors(
                self.primary,
                self.device,
                lba,
                chunk_size as u16,
                &mut buffer[..chunk_bytes],
            )?;

            for i in 0..chunk_size {
                let src_offset = i * 512;
                blocks[blocks_read + i]
                    .as_mut()
                    .copy_from_slice(&buffer[src_offset..src_offset + 512]);
            }

            blocks_read += chunk_size;
        }

        Ok(())
    }

    fn write(&self, blocks: &[Block], start_block_idx: BlockIdx) -> Result<(), Self::Error> {
        let total_blocks = blocks.len();
        let mut blocks_written = 0;
        let mut buffer = SECTOR_BUFFER.lock();

        while blocks_written < total_blocks {
            let blocks_remaining = total_blocks - blocks_written;
            let chunk_size = blocks_remaining.min(256);
            let chunk_bytes = chunk_size * 512;

            let lba = start_block_idx.0 as u64 + blocks_written as u64;

            for i in 0..chunk_size {
                let dst_offset = i * 512;
                buffer[dst_offset..dst_offset + 512]
                    .copy_from_slice(blocks[blocks_written + i].as_ref());
            }

            write_sectors(self.primary, self.device, lba, &buffer[..chunk_bytes])?;
            blocks_written += chunk_size;
        }

        Ok(())
    }

    fn num_blocks(&self) -> Result<BlockCount, Self::Error> {
        Ok(BlockCount(self.block_count))
    }
}
