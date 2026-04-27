use std::{fs::File, sync::{Arc, RwLock}};

use ahash::RandomState;
use scc::HashMap;

use crate::{DbBuilder, stats::{Statistics, StatsSnapshot}, storage::metadata::Metadata};

pub mod builder;
pub mod ttl;
pub mod init;

pub struct FunKV {
    pub(super) hash_table: HashMap<Vec<u8>, Arc<Record>, RandomState>,  // TODO: Record

    pub(super) tree: Arc<SkipMap<Vec<u8>, Arc<Record>>>,    // TODO: SkipMap

    pub(super) stats: Arc<Statistics>,

    pub(super) write_buffer: Option<Arc<WriterBuffer>>,    // TODO: WriteBuffer

    pub(super) free_space: Arc<RwLock<FreeSpaceManager>>,   // TODO: FreeSpaceManager

    pub(super)  _metadata: Arc<RwLock<Metadata>>,

    pub(super) persistency: bool,
    pub(super) enable_caching: bool,
    pub(super) max_memory: Option<usize>,

    pub(super) cache: Option<Arc<ClockCache>>,      // TODO: ClockCache
    #[cfg(unix)]
    pub(super) device_fd: Option<i32>,
    pub(super) device_size: u64,
    pub(super) device_file: Option<File>,

    pub(super) disk_io: Option<Arc<RwLock<DiskIO>>>,        // TODO: DiskIO

    pub(super) enable_ttl: bool,
    pub(super) ttl: Arc<RwLock<Option<TtlSweeper>>>,        // TODO: TtlSweeper
}

impl FunKV {
    pub fn builder() -> DbBuilder {
        DbBuilder::new()
    }

    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.hash_table.contains(key)
    }

    pub fn len(&self) -> usize {
        self.stats
            .record_count
            .load(std::sync::atomic::Ordering::Acquire)
            as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn memory_usage(&self) -> usize {
        self.stats
            .memory_usage
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn stats(&self) -> StatsSnapshot {
        self.stats.snapshot()
    }

    pub fn flush(&self) {
        self.flush_all()    // TODO: flush_all
    }
}