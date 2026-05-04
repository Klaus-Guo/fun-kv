use std::{mem, time::{SystemTime, UNIX_EPOCH}};

use crate::core::record::Record;

use super::FunKV;

impl FunKV {
    pub(super) fn get_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    pub(super) fn calculate_record_size(&self, key_len: usize, value_len: usize) -> usize {
        mem::size_of::<Record>() + key_len + value_len
    }
}