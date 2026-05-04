use std::{fs::File, sync::Arc};

use ahash::RandomState;
use crossbeam_skiplist::SkipMap;
use parking_lot::RwLock;
use scc::HashMap;

use crate::{
    core::{cache::Cache, record::Record, ttl::TtlSweeper},
    stats::Statistics,
    storage::{
        free_space_manager::FreeSpaceManager, io::DiskIO, metadata::Metadata,
        write_buffer::WriteBuffer,
    },
};

pub mod builder;
pub mod cache;
pub mod init;
pub mod record;
pub mod ttl;
pub mod persistence;
pub mod recovery;
pub mod operations;
pub mod internal;

pub struct FunKV {
    pub(super) hash_table: HashMap<Vec<u8>, Arc<Record>, RandomState>,

    pub(super) tree: Arc<SkipMap<Vec<u8>, Arc<Record>>>,

    pub(super) stats: Arc<Statistics>,

    pub(super) write_buffer: Option<Arc<WriteBuffer>>,

    pub(super) free_space: Arc<RwLock<FreeSpaceManager>>,

    pub(super) _metadata: Arc<RwLock<Metadata>>,

    pub(super) persistency: bool,
    pub(super) enable_caching: bool,
    pub(super) max_memory: Option<usize>,

    pub(super) cache: Option<Arc<Cache>>,
    #[cfg(unix)]
    pub(super) file_fd: Option<i32>,
    pub(super) persistence_size: u64,
    pub(super) persistence_file: Option<File>,

    pub(super) disk_io: Option<Arc<RwLock<DiskIO>>>,

    pub(super) enable_ttl: bool,
    pub(super) ttl: Arc<RwLock<Option<TtlSweeper>>>,
}