use std::sync::Arc;

use ahash::RandomState;
use scc::HashMap;

use crate::{DbBuilder, core::record::Record, stats::StatsSnapshot, storage::write_buffer::WriteBuffer};

use super::FunKV;

impl FunKV {
    pub(super) fn builder() -> DbBuilder {
        DbBuilder::new()
    }

    pub(super) fn contains_key(&self, key: &[u8]) -> bool {
        self.hash_table.contains_sync(key)
    }

    pub(super) fn len(&self) -> usize {
        self.stats
            .record_count
            .load(std::sync::atomic::Ordering::Acquire) as usize
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(super) fn memory_usage(&self) -> usize {
        self.stats
            .memory_usage
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub(super) fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    pub(super) fn flush(&self) {
        self.flush_all()
    }

    pub(crate) fn get_hash_table(&self) -> &HashMap<Vec<u8>, Arc<Record>, RandomState> {
        &self.hash_table
    } 

    pub(crate) fn remove_from_tree(&self, key: &[u8]) {
        self.tree.remove(key);
    }

    pub(crate) fn get_write_buffer(&self) -> Option<&Arc<WriteBuffer>> {
        self.write_buffer.as_ref()
    }
}
