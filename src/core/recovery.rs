use std::sync::{Arc, atomic::Ordering};

use bytes::Bytes;

use crate::{constants::*, core::record::Record, error::{DbError, Result}, storage::{format::get_format, metadata::Metadata}};

use super::FunKV;

impl FunKV {
    pub(super) fn load_indexes(&mut self) -> Result<()> {
        if !self.persistency {
            return Ok(());
        }

        if let Some(ref disk_io) = self.disk_io {
            let metadata_bytes = disk_io.read().read_metadata()?;

            if metadata_bytes.len() >= SIGNATURE_SIZE {
                let signature = &metadata_bytes[..SIGNATURE_SIZE];

                if signature == SIGNATURE {
                    if let Some(metadata) = Metadata::from_bytes(&metadata_bytes) {
                        *self._metadata.write() = metadata;
                    }

                    self.scan_and_rebuild_indexes()?;
                }
            }
        }

        Ok(())
    }

    fn scan_and_rebuild_indexes(&mut self) -> Result<()> {
        if !self.persistency || self.persistence_size == 0 {
            return Ok(());
        }

        let disk_io = self.disk_io.as_ref().ok_or(DbError::NoDevice)?;

        let metadata_version = self._metadata.read().version;
        let format = get_format(metadata_version);

        let total_sectors = self.persistence_size / BLOCK_SIZE as u64;
        let mut sector: u64 = 1;
        let mut _records_loaded = 0;
        let mut occupied_sectors = Vec::new();

        while sector < total_sectors {
            let data = match disk_io.read().read_sectors_sync(sector, 1) {
                Ok(d) => d,
                Err(_) => {
                    sector += 1;
                    continue;
                }
            };

            if data.len() < SECTOR_HEADER_SIZE {
                sector += 1;
                continue;
            }

            if data.len() >= 8 && &data[..8] == DELETED_MARKER {
                sector += 1;
                continue;
            }

            let marker = u16::from_le_bytes([data[0], data[1]]);
            let seq_num = u16::from_le_bytes([data[2], data[3]]);

            if marker != SECTOR_MARKER || seq_num != 0 {
                sector += 1;
                continue;
            }

            if data.len() < SECTOR_HEADER_SIZE + 2 {
                sector += 1;
                continue;
            }

            let (key, value_len, timestamp, ttl) = match format.parse_record(&data) {
                Some(parsed) => parsed,
                None => {
                    sector += 1;
                    continue;
                }
            };

            if key.is_empty() || key.len() > MAX_KEY_SIZE {
                sector += 1;
                continue;
            }

            let total_size = format.total_size(key.len(), value_len);
            let sectors_needed = total_size.div_ceil(BLOCK_SIZE);

            let mut record = Record::new(key.clone(), Bytes::from(Vec::new()), timestamp);
            record.sector.store(sector, Ordering::Release);
            record.value_len = value_len;
            record.ttl.store(ttl, Ordering::Release);
            record.clear_value();

            if self.enable_ttl && ttl > 0 && self.get_timestamp() > ttl {
                sector += sectors_needed as u64;
                continue;
            }

            let record_arc = Arc::new(record);
            let key_len = key.len();
            self.hash_table.upsert_sync(key.clone(), Arc::clone(&record_arc));
            self.tree.insert(key, Arc::clone(&record_arc));

            self.stats.record_count.fetch_add(1, Ordering::AcqRel);
            let record_size = self.calculate_record_size(key_len, value_len);

            self.stats
                .memory_usage
                .fetch_add(record_size, Ordering::AcqRel);

            self.stats
                .disk_usage
                .fetch_add((sectors_needed * BLOCK_SIZE) as u64, Ordering::AcqRel);

            for i in 0..sectors_needed {
                occupied_sectors.push(sector + i as u64);
            }

            _records_loaded += 1;
            sector += sectors_needed as u64;
        }

        occupied_sectors.sort_unstable();

        let mut last_end = DATA_START_BLOCK;

        for &occupied_start in &occupied_sectors {
            if occupied_start > last_end {
                self.free_space
                    .write()
                    .release_sectors(last_end, occupied_start - last_end)?;
            }
            last_end = occupied_start + 1;
        }

        if last_end < total_sectors {
            self.free_space
                .write()
                .release_sectors(last_end, total_sectors - last_end)?;
        }
        
        Ok(())
    }
}