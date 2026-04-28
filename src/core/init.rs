use std::sync::{Arc, RwLock};

use ahash::RandomState;
use scc::HashMap;

use crate::{DbConfig, core::{FunKV, cache::Cache}, error::Result, stats::Statistics, storage::{free_space_manager::FreeSpaceManager, metadata::Metadata}};

impl FunKV {
    pub fn build_with_config(config: DbConfig) -> Result<Self> {
        let hash_table = HashMap::with_capacity_and_hasher(1 << config.hash_bits, RandomState::new());

        let free_space = Arc::new(RwLock::new(FreeSpaceManager::new()));

        let metadata = Arc::new(RwLock::new(Metadata::new()));

        let stats = Arc::new(Statistics::new());

        let cache = if config.enable_caching {
            Some(Arc::new(Cache::new(stats)))
        } else {
            None
        };

        let mut store = Self {
            // TODO: param
        };

        if config.persistency {
            // TODO: persistent procedure
        }

        Ok(store)
    }
}