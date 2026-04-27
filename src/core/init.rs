use std::{hash::RandomState, sync::{Arc, RwLock}};

use scc::HashMap;

use crate::{DbConfig, core::{FunKV, metadata::Metadata}, error::Result, stats::Statistics};

impl FunKV {
    pub fn build_with_config(config: DbConfig) -> Result<Self> {
        let hash_table = HashMap::with_capacity_and_hasher(1 << config.hash_bits, RandomState::new());

        let free_space = Arc::new(RwLock::new(FreeSpaceManager::new()));    // TODO: FreeSpaceManager

        let metadata = Arc::new(RwLock::new(Metadata::new()));      // TODO: Metadata

        let stats = Arc::new(Statistics::new());

        let cache = if config.enable_caching {
            Some(Arc::new(Cache::new(stats)))       // TODO: Cache
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