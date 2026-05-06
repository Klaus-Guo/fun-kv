use std::{
    mem,
    sync::{Arc, atomic::Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    constants::*,
    core::record::Record,
    error::{DbError, Result},
};

use super::FunKV;

impl FunKV {
    pub fn insert(&self, key: &[u8], value: &[u8]) -> Result<bool> {
        self.insert_with_timestamp(key, value, None)
    }

    pub fn insert_with_timestamp(
        &self,
        key: &[u8],
        value: &[u8],
        timestamp: Option<u64>,
    ) -> Result<bool> {
        self.insert_with_timestamp_and_ttl_internal(key, value, timestamp, 0)
    }

    pub fn get(&self, key: &[u8]) -> Result<Vec<u8>> {
        let start = Instant::now();
        self.validate_key(key)?;

        if self.enable_caching {
            if let Some(ref cache) = self.cache {
                if let Some(value) = cache.get(key) {
                    self.stats
                        .record_get(start.elapsed().as_nanos() as u64, true);
                    return Ok(value.to_vec());
                }
            }
        }

        let record = self
            .hash_table
            .read_sync(key, |_, v| v.clone())
            .ok_or(DbError::KeyNotFound)?;

        if self.enable_ttl {
            let ttl = record.ttl.load(Ordering::Relaxed);

            if ttl > 0 {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;

                if now > ttl {
                    self.stats.ttl_expired_lazy.fetch_add(1, Ordering::Relaxed);
                    return Err(DbError::KeyNotFound);
                }
            }
        }

        let (value, cache_hit) = if let Some(val) = record.get_value() {
            (val, true)
        } else {
            (self.load_value_from_disk(&record)?, false)
        };

        if self.enable_caching {
            if let Some(ref cache) = self.cache {
                cache.insert(key.to_vec(), value.clone());
            }
        }

        self.stats
            .record_get(start.elapsed().as_nanos() as u64, cache_hit);

        Ok(value)
    }

    pub(super) fn insert_with_timestamp_and_ttl_internal(
        &self,
        key: &[u8],
        value: &[u8],
        timestamp: Option<u64>,
        ttl: u64,
    ) -> Result<bool> {
        let start = Instant::now();
        let timestamp = match timestamp {
            Some(0) | None => self.get_timestamp(),
            Some(ts) => ts,
        };
        self.validate_key_value(key, value)?;

        let is_update = self.hash_table.contains_sync(key);
        let existing_record = self.hash_table.read_sync(key, |_, v| v.clone());

        if let Some(existing_record) = existing_record {
            let existing_ts = existing_record.timestamp;
            let existing_clone = existing_record;

            if timestamp < existing_ts {
                return Err(DbError::OlderTimestamp);
            }

            return self.update_record_with_ttl(&existing_clone, value, timestamp, ttl);
        }

        let record_size = Self::calculate_record_size(key.len(), value.len());
        if !self.check_memory_limit(record_size) {
            return Err(DbError::OutOfMemory);
        }

        let record = if ttl > 0 && self.enable_ttl {
            self.stats.keys_with_ttl.fetch_add(1, Ordering::Relaxed);
            Arc::new(Record::new_with_ttl(
                key.to_vec(),
                value.to_vec(),
                timestamp,
                ttl,
            ))
        } else {
            Arc::new(Record::new(key.to_vec(), value.to_vec(), timestamp))
        };

        let key_vec = record.key.clone();

        self.hash_table
            .upsert_sync(key_vec.clone(), Arc::clone(&record));

        self.tree.insert(key_vec, Arc::clone(&record));

        self.stats.record_count.fetch_add(1, Ordering::AcqRel);
        self.stats
            .memory_usage
            .fetch_add(record_size, Ordering::AcqRel);
        self.stats
            .record_insert(start.elapsed().as_nanos() as u64, is_update);

        if self.persistency {
            if let Some(ref write_buffer) = self.write_buffer {
                if let Err(_e) = write_buffer.add_write(Operation::Insert, record, 0) {
                    // data is already inserted into memory
                }
            }
        }

        Ok(!is_update)
    }

    pub(super) fn update_record_with_ttl(
        &self,
        old_record: &Record,
        value: &[u8],
        timestamp: u64,
        ttl: u64,
    ) -> Result<bool> {
        let new_record = if ttl > 0 && self.enable_ttl {
            Arc::new(Record::new_with_ttl(
                old_record.key.clone(),
                value.to_vec(),
                timestamp,
                ttl,
            ))
        } else {
            Arc::new(Record::new(
                old_record.key.clone(),
                value.to_vec(),
                timestamp,
            ))
        };

        let old_value_len = old_record.value_len;
        let old_size = old_record.calculate_size();
        let new_size = Self::calculate_record_size(old_record.key.len(), value.len());

        let old_record_arc =
            if let Some(entry) = self.hash_table.read_sync(&old_record.key, |_, v| v.clone()) {
                entry
            } else {
                return Err(DbError::KeyNotFound);
            };

        let key_vec = new_record.key.clone();

        self.hash_table
            .upsert_sync(key_vec.clone(), Arc::clone(&new_record));
        self.tree.insert(key_vec.clone(), Arc::clone(&new_record));

        if new_size > old_size {
            self.stats
                .memory_usage
                .fetch_add(new_size - old_size, Ordering::AcqRel);
        } else {
            self.stats
                .memory_usage
                .fetch_sub(old_size - new_size, Ordering::AcqRel);
        }

        if self.persistency {
            if self.enable_caching {
                if let Some(ref cache) = self.cache {
                    cache.remove(&key_vec);
                }
            }

            if let Some(ref write_buffer) = self.write_buffer {
                if let Err(e) = write_buffer.add_write(Operation::Update, new_record, old_value_len)
                {
                    let _ = e;
                }

                if let Err(e) =
                    write_buffer.add_write(Operation::Delete, old_record_arc, old_value_len)
                {
                    let _ = e;
                }
            }
        }

        Ok(false)
    }

    pub(super) fn get_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    pub(super) fn calculate_record_size(key_len: usize, value_len: usize) -> usize {
        mem::size_of::<Record>() + key_len + value_len
    }

    pub(super) fn validate_key(&self, key: &[u8]) -> Result<()> {
        if key.is_empty() || key.len() > MAX_KEY_SIZE {
            return Err(DbError::InvalidKeySize);
        }

        Ok(())
    }

    pub(super) fn validate_key_value(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.validate_key(key)?;

        if value.is_empty() || value.len() > MAX_VALUE_SIZE {
            return Err(DbError::InvalidValueSize);
        }

        Ok(())
    }

    pub(super) fn check_memory_limit(&self, size: usize) -> bool {
        match self.max_memory {
            Some(limit) => {
                let current = self.stats.memory_usage.load(Ordering::Acquire);
                current + size <= limit
            }
            None => true,
        }
    }
}
