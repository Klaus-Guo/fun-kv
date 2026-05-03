use std::sync::Arc;

use ahash::RandomState;
use crossbeam_skiplist::SkipMap;
use parking_lot::RwLock;
use scc::HashMap;

use crate::{
    DbConfig,
    constants::*,
    core::{FunKV, cache::Cache},
    error::Result,
    stats::Statistics,
    storage::{
        free_space_manager::FreeSpaceManager, metadata::Metadata, write_buffer::WriteBuffer,
    },
};

impl FunKV {
    pub fn new(file_path: Option<String>) -> Result<Self> {
        let persistency = !file_path.is_none();
        let config = DbConfig {
            hash_bits: DEFAULT_HASH_BITS,
            persistency,
            enable_caching: persistency,
            enable_ttl: false,
            max_memory: Some(DEFAULT_MAX_MEMORY),
            ttl_config: None,
            file_path,
            file_size: None,
        };

        Self::build_with_config(config)
    }

    pub fn build_with_config(config: DbConfig) -> Result<Self> {
        let hash_table =
            HashMap::with_capacity_and_hasher(1 << config.hash_bits, RandomState::new());

        let free_space = Arc::new(RwLock::new(FreeSpaceManager::new()));

        let metadata = Arc::new(RwLock::new(Metadata::new()));

        let stats = Arc::new(Statistics::new());

        let cache = if config.enable_caching {
            Some(Arc::new(Cache::new(stats)))
        } else {
            None
        };

        let mut store = Self {
            hash_table,
            tree: Arc::new(SkipMap::new()),
            stats: stats.clone(),
            write_buffer: None,
            free_space: free_space.clone(),
            _metadata: metadata,
            persistency: config.persistency,
            enable_caching: config.enable_caching,
            max_memory: config.max_memory,
            cache,
            #[cfg(unix)]
            device_fd: None,
            device_size: 0,
            device_file: None,
            disk_io: None,
            enable_ttl: config.enable_ttl,
            ttl: Arc::new(RwLock::new(None)),
        };

        if config.persistency {
            store.open_device(&config.file_path, config.file_size)?;
            store.load_indexes()?;

            if let Some(ref disk_io) = store.disk_io {
                let metadata_version = store._metadata.read().version;
                let mut write_buffer =
                    WriteBuffer::new(disk_io.clone(), free_space, stats.clone(), metadata_version);
                let num_worker = (num_cpus::get() / 2).max(1);
                write_buffer.start_workers(num_worker);
                store.write_buffer = Some(Arc::new(write_buffer));
            }
        }

        Ok(store)
    }
}
