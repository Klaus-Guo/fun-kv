use std::sync::atomic::Ordering;

use libc::time;

use crate::{constants::*, core::record::Record};

pub trait RecordFormat: Send + Sync {
    fn record_header_size(&self, key_len: usize) -> usize;

    fn total_size(&self, key_len: usize, value_len: usize) -> usize;

    fn serialize_record(&self, record: &Record, include_value: bool) -> Vec<u8>;

    fn parse_record(&self, data: &[u8]) -> Option<(Vec<u8>, usize, u64, u64)>;
}

pub struct DefaultFormat;

impl RecordFormat for DefaultFormat {
    fn record_header_size(&self, key_len: usize) -> usize {
        SECTOR_HEADER_SIZE + 2 + key_len + 8 + 8 + 8 //header + key_len(2) + key + value_len(8) + timestamp(8) + ttl(8)
    }

    fn total_size(&self, key_len: usize, value_len: usize) -> usize {
        self.record_header_size(key_len) + value_len
    }

    fn serialize_record(&self, record: &Record, include_value: bool) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.total_size(record.key.len(), record.value_len));

        data.extend_from_slice(&(record.key.len() as u16).to_le_bytes());

        data.extend_from_slice(&record.key);

        data.extend_from_slice(&(record.value_len as u64).to_le_bytes());

        data.extend_from_slice(&record.timestamp.to_le_bytes());

        data.extend_from_slice(&record.ttl.load(Ordering::Acquire).to_le_bytes());

        if include_value {
            if let Some(value) = record.value.read().as_ref() {
                data.extend_from_slice(value);
            }
        }

        data
    }

    fn parse_record(&self, data: &[u8]) -> Option<(Vec<u8>, usize, u64, u64)> {
        if data.len() < SECTOR_HEADER_SIZE + 2 {
            return None;
        }

        let mut offset = SECTOR_HEADER_SIZE + 2;
        let key_len =
            u16::from_le_bytes(data[SECTOR_HEADER_SIZE..offset].try_into().ok()?) as usize;

        if offset + key_len + 24 > data.len() {
            return None;
        }

        let key = data[offset..offset + key_len].to_vec();
        offset += key_len;

        let value_len = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?) as usize;
        offset += 8;

        let timestamp = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;

        let ttl = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);

        Some((key, value_len, timestamp, ttl))
    }
}

pub fn get_format(version: u32) -> Box<dyn RecordFormat> {
    match version {
        _ => Box::new(DefaultFormat),
    }
}
