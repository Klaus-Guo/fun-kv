use std::collections::BTreeMap;

use crate::{
    constants::*,
    error::{DbError, Result},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorStat {
    pub start: u64,
    pub size: u64,
}

pub struct FreeSpaceManager {
    /// Sorted by (size, start)
    by_size: BTreeMap<(u64, u64), SectorStat>,

    /// Sorted by start address
    by_start: BTreeMap<u64, SectorStat>,

    /// Total free space in bytes
    total_free: u64,

    /// Persistence size in bytes
    persistence_size: u64,

    fragmentation_percent: u32,
}

impl FreeSpaceManager {
    pub fn new() -> Self {
        Self {
            by_size: BTreeMap::new(),
            by_start: BTreeMap::new(),
            total_free: 0,
            persistence_size: 0,
            fragmentation_percent: 0,
        }
    }

    // TODO: Test when persistence_size is 0
    pub fn initialize(&mut self, persistence_size: u64) -> Result<()> {
        self.persistence_size = persistence_size;

        let total_sectors = persistence_size / BLOCK_SIZE as u64;

        if total_sectors <= METADATA_BLOCK_SIZE {
            return Err(DbError::InvalidDevice);
        }

        let free_sectors = total_sectors - METADATA_BLOCK_SIZE;
        self.insert_free_space(SectorStat {
            start: METADATA_BLOCK_SIZE,
            size: free_sectors,
        })?;

        Ok(())
    }

    pub fn set_persistence_size(&mut self, persistence_size: u64) {
        self.persistence_size = persistence_size;
    }

    pub fn allocate_sectors(&mut self, sectors: u64) -> Result<u64> {
        if sectors == 0 {
            return Err(DbError::InvalidArgument);
        }

        let mut best_fit = None;

        // Search from first available size
        for ((size, start), stat) in &self.by_size {
            if *size >= sectors {
                best_fit = Some((*size, *start, stat.clone()));
                break;
            }
        }

        if let Some((size, start, stat)) = best_fit {
            if !self.is_valid_free_space(&stat) {
                return Err(DbError::CorruptedData);
            }

            self.by_size.remove(&(size, start));
            self.by_start.remove(&stat.start);

            let allocated_start = stat.start;

            if stat.size > sectors {
                let remaining = SectorStat {
                    start: stat.start + sectors,
                    size: stat.size - sectors,
                };

                if let Err(e) = self.insert_free_space(remaining) {
                    let _ = self.insert_free_space(stat);
                    return Err(e);
                }
            }

            self.total_free -= sectors * BLOCK_SIZE as u64;
            self.update_fragmentation();

            Ok(allocated_start)
        } else {
            Err(DbError::OutOfSpace)
        }
    }

    pub fn release_sectors(&mut self, start: u64, size: u64) -> Result<()> {
        if start == 0 || size == 0 {
            return Err(DbError::InvalidArgument);
        }

        if !self.is_valid_sector_range(start, size) {
            return Err(DbError::InvalidArgument);
        }

        let merged = self.try_merge_spaces(start, size)?;

        self.insert_free_space(merged)?;

        self.update_fragmentation();

        Ok(())
    }

    fn try_merge_spaces(&mut self, start: u64, size: u64) -> Result<SectorStat> {
        let end = start + size;
        let mut merged_start = start;
        let mut merged_size = size;

        let mut prev = None;

        if let Some((&s, stat)) = self.by_start.range(..start).rev().next() {
            if s + stat.size == start {
                prev = Some(stat.clone());
            }
        }

        let next = self.by_start.get(&end).cloned();

        if let Some(prev_stat) = prev {
            self.btree_remover(&prev_stat);

            merged_start = prev_stat.start;
            merged_size = prev_stat.size;
        }

        if let Some(next_stat) = next {
            self.btree_remover(&next_stat);

            merged_size += next_stat.size;
        }

        Ok(SectorStat {
            start: merged_start,
            size: merged_size,
        })
    }

    fn btree_remover(&mut self, stat: &SectorStat) {
        self.by_size.remove(&(stat.size, stat.start));
        self.by_start.remove(&stat.start);

        self.total_free -= stat.size * BLOCK_SIZE as u64;
    }

    fn insert_free_space(&mut self, stat: SectorStat) -> Result<()> {
        if stat.size == 0 {
            return Err(DbError::InvalidArgument);
        }

        if !self.is_valid_free_space(&stat) {
            return Err(DbError::InvalidArgument);
        }

        if self.by_start.contains_key(&stat.start) {
            return Err(DbError::DuplicateKey);
        }

        self.total_free += stat.size * BLOCK_SIZE as u64;

        self.by_size.insert((stat.size, stat.start), stat.clone());
        self.by_start.insert(stat.start, stat);

        Ok(())
    }

    fn is_valid_free_space(&self, stat: &SectorStat) -> bool {
        // reserved for metadata
        if stat.start < METADATA_BLOCK_SIZE {
            return false;
        }

        self.is_valid_sector_range(stat.start, stat.size)
    }

    fn is_valid_sector_range(&self, start: u64, count: u64) -> bool {
        if self.persistence_size > 0 {
            let persistence_sectors = self.persistence_size / BLOCK_SIZE as u64;

            if start >= persistence_sectors {
                return false;
            }
            if start + count > persistence_sectors {
                return false;
            }
        }

        true
    }

    fn update_fragmentation(&mut self) {
        if self.total_free == 0 {
            self.fragmentation_percent = 0;
            return;
        }

        let largest = self.get_largest_free_chunk();

        // Calculate fragmentation as percentage of free space not in largest chunk
        if self.total_free > 0 {
            let fragmented = self.total_free - largest;
            self.fragmentation_percent = ((fragmented * 100) / self.total_free) as u32;
        }
    }

    pub fn get_largest_free_chunk(&self) -> u64 {
        self.by_size
            .iter()
            .next_back()
            .map(|(_, stat)| stat.size * BLOCK_SIZE as u64)
            .unwrap_or(0)
    }

    pub fn get_free_chunks_count(&self) -> usize {
        self.by_start.len()
    }

    pub fn get_total_free(&self) -> u64 {
        self.total_free
    }

    pub fn get_fragmentation_percent(&self) -> u32 {
        self.fragmentation_percent
    }
}

impl Default for FreeSpaceManager {
    fn default() -> Self {
        Self::new()
    }
}
