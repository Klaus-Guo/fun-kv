use std::{
    collections::VecDeque, sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicUsize},
    }, thread::JoinHandle, time::Instant
};

use crossbeam_channel::Sender;
use crossbeam_utils::CachePadded;
use parking_lot::{Mutex, RwLock};

use crate::{constants::Operation, core::record::Record, stats::Statistics, storage::free_space_manager::FreeSpaceManager, error::Result};

#[repr(align(64))]
pub struct ShardedWriteBuffer {
    buffer: Mutex<VecDeque<WriteEntry>>,

    count: AtomicUsize,

    size: AtomicUsize,
}

pub struct WriteEntry {
    pub operation: Operation,
    pub record: Arc<Record>,
    pub old_value_len: usize,
    pub work_status: AtomicU32,
    pub retry_count: AtomicU32,
    pub timestamp: Instant,
}

pub struct WriteBuffer {
    sharded_buffers: Arc<Vec<CachePadded<ShardedWriteBuffer>>>,
    disk_io: Arc<RwLock<DiskIO>>,
    free_space: Arc<RwLock<FreeSpaceManager>>,
    worker_channels: Vec<Sender<FlushRequest>>,
    worker_handles: Vec<JoinHandle<()>>,
    periodic_flush_handle: Option<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    stats: Arc<Statistics>,
    format_version: u32,
}

#[derive(Debug)]
struct FlushRequest {
    response: Option<Sender<Result<()>>>,
}