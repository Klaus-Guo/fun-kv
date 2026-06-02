use std::{
    cell::Cell,
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, Sender, bounded};
use crossbeam_utils::CachePadded;
use parking_lot::{Mutex, RwLock};

use crate::{
    constants::*,
    core::record::Record,
    error::{DbError, Result},
    stats::Statistics,
    storage::{
        format::{RecordFormat, get_format},
        free_space_manager::FreeSpaceManager,
        io::DiskIO,
    },
};

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

struct WorkerContext {
    worker_id: usize,
    disk_io: Arc<RwLock<DiskIO>>,
    free_space: Arc<RwLock<FreeSpaceManager>>,
    sharded_buffers: Arc<Vec<CachePadded<ShardedWriteBuffer>>>,
    shutdown: Arc<AtomicBool>,
    stats: Arc<Statistics>,
    format_version: u32,
}

impl ShardedWriteBuffer {
    fn new(_shared_id: usize) -> Self {
        Self {
            buffer: Mutex::new(VecDeque::new()),
            count: AtomicUsize::new(0),
            size: AtomicUsize::new(0),
        }
    }

    fn add_entry(&self, entry: WriteEntry) -> Result<()> {
        let entry_size = entry.record.calculate_size();

        let mut buffer = self.buffer.lock();
        buffer.push_back(entry);

        self.count.fetch_add(1, Ordering::AcqRel);
        self.size.fetch_add(entry_size, Ordering::AcqRel);

        Ok(())
    }

    fn drain_entries(&self) -> Vec<WriteEntry> {
        let mut buffer = self.buffer.lock();
        let entries = buffer.drain(..).collect();

        self.count.store(0, Ordering::Release);
        self.size.store(0, Ordering::Release);

        entries
    }

    fn is_full(&self) -> bool {
        self.count.load(Ordering::Acquire) >= WRITE_BUFFER_COUNT
            || self.size.load(Ordering::Acquire) >= WRITE_BUFFER_SIZE
    }
}

impl WriteBuffer {
    pub fn new(
        disk_io: Arc<RwLock<DiskIO>>,
        free_space: Arc<RwLock<FreeSpaceManager>>,
        stats: Arc<Statistics>,
        format_version: u32,
    ) -> Self {
        let num_shards = (num_cpus::get() / 2).max(1);

        let sharded_buffers = Arc::new(
            (0..num_shards)
                .map(|shard_id| CachePadded::new(ShardedWriteBuffer::new(shard_id)))
                .collect(),
        );

        Self {
            sharded_buffers,
            disk_io,
            free_space,
            worker_channels: Vec::new(),
            worker_handles: Vec::new(),
            periodic_flush_handle: None,
            shutdown: Arc::new(AtomicBool::new(false)),
            stats,
            format_version,
        }
    }

    pub fn add_write(
        &self,
        operation: Operation,
        record: Arc<Record>,
        old_value_len: usize,
    ) -> Result<()> {
        if self.shutdown.load(Ordering::Acquire) {
            return Err(DbError::ShuttingDown);
        }

        let entry = WriteEntry {
            operation,
            record,
            old_value_len,
            work_status: AtomicU32::new(0),
            retry_count: AtomicU32::new(0),
            timestamp: Instant::now(),
        };

        let shard_id = self.get_shard_id();
        let buffer = &self.sharded_buffers[shard_id];

        buffer.add_entry(entry)?;
        self.stats.record_write_buffered();

        if buffer.is_full() && shard_id < self.worker_channels.len() {
            let request = FlushRequest { response: None };
            let _ = self.worker_channels[shard_id].try_send(request);
        }

        Ok(())
    }

    pub fn start_workers(&mut self, num_workers: usize) {
        let num_shards = self.sharded_buffers.len();
        let actual_workers = num_workers.min(num_shards);

        let mut receivers = Vec::new();
        for _ in 0..actual_workers {
            let (tx, rx) = bounded(2);
            self.worker_channels.push(tx);
            receivers.push(rx);
        }

        for worker_id in 0..actual_workers {
            let ctx = WorkerContext {
                worker_id,
                disk_io: self.disk_io.clone(),
                free_space: self.free_space.clone(),
                sharded_buffers: self.sharded_buffers.clone(),
                shutdown: self.shutdown.clone(),
                stats: self.stats.clone(),
                format_version: self.format_version,
            };

            let flush_rx = receivers.pop().unwrap();

            let handle = thread::spawn(move || {
                write_buffer_worker(ctx, flush_rx);
            });

            self.worker_handles.push(handle);
        }

        let worker_channels = self.worker_channels.clone();
        let shutdown = self.shutdown.clone();
        let sharded_buffers = self.sharded_buffers.clone();

        let periodic_handle = thread::spawn(move || {
            let interval = WRITE_BUFFER_FLUSH_INTERVAL;

            while !shutdown.load(Ordering::Acquire) {
                thread::sleep(interval);

                for (shard_id, buffer) in sharded_buffers.iter().enumerate() {
                    let count = buffer.count.load(Ordering::Relaxed);
                    if count > 0 && shard_id < worker_channels.len() {
                        let request = FlushRequest { response: None };
                        let channel_idx = worker_channels.len() - 1 - shard_id;
                        let _ = worker_channels[channel_idx].try_send(request);
                    }
                }
            }
        });

        self.periodic_flush_handle = Some(periodic_handle);
    }

    pub fn force_flush(&self) -> Result<()> {
        let mut responses = Vec::new();

        for worker_tx in &self.worker_channels {
            let (tx, rx) = bounded(1);
            let request = FlushRequest { response: Some(tx) };

            worker_tx.send(request).map_err(|_| DbError::ChannelError)?;
            responses.push(rx);
        }

        for rx in responses {
            rx.recv().map_err(|_| DbError::ChannelError)??;
        }

        Ok(())
    }

    pub fn initiate_shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    pub fn complete_shutdown(&mut self) {
        use std::time::Duration;

        self.shutdown.store(true, Ordering::Release);

        if let Some(handle) = self.periodic_flush_handle.take() {
            let (tx, rx) = crossbeam_channel::bounded(1);
            thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });

            if rx.recv_timeout(Duration::from_secs(5)).is_err() {
                // Timeout waiting
            }
        }

        for handle in self.worker_handles.drain(..) {
            let _ = handle.join();
        }
    }

    #[inline]
    fn get_shard_id(&self) -> usize {
        thread_local! {
            static THREAD_SHARD_ID: Cell<Option<usize>> = const { Cell::new(None) };
        }

        THREAD_SHARD_ID.with(|id| {
            if let Some(cpu_id) = id.get() {
                cpu_id
            } else {
                use std::collections::hash_map::RandomState;
                use std::hash::BuildHasher;

                let shard_id = RandomState::new().hash_one(std::thread::current().id()) as usize
                    % self.sharded_buffers.len();
                id.set(Some(shard_id));
                shard_id
            }
        })
    }
}

impl Drop for WriteBuffer {
    fn drop(&mut self) {
        if !self.shutdown.load(Ordering::Acquire) {
            self.complete_shutdown();
        }
    }
}

fn write_buffer_worker(ctx: WorkerContext, flush_rx: Receiver<FlushRequest>) {
    let worker_id = ctx.worker_id;
    let format = get_format(ctx.format_version);

    loop {
        if ctx.shutdown.load(Ordering::Acquire) {
            break;
        }

        let req = match flush_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(req) => req,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                continue;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
        };

        if worker_id < ctx.sharded_buffers.len() {
            let buffer = &ctx.sharded_buffers[worker_id];
            let entries = buffer.drain_entries();

            if !entries.is_empty() {
                let result = process_write_batch(
                    &ctx.disk_io,
                    &ctx.free_space,
                    entries,
                    &ctx.stats,
                    format.as_ref(),
                );

                ctx.stats.flush_count.fetch_add(1, Ordering::Relaxed);

                if let Some(tx) = req.response {
                    let _ = tx.send(result);
                }
            } else if let Some(tx) = req.response {
                let _ = tx.send(Ok(()));
            }
        }
    }

    if ctx.shutdown.load(Ordering::Acquire) && worker_id < ctx.sharded_buffers.len() {
        let buffer = &ctx.sharded_buffers[worker_id];
        let final_entries = buffer.drain_entries();

        if !final_entries.is_empty() {
            let _ = process_write_batch(
                &ctx.disk_io,
                &ctx.free_space,
                final_entries,
                &ctx.stats,
                format.as_ref(),
            );
        }
    }
}

fn process_write_batch(
    disk_io: &Arc<RwLock<DiskIO>>,
    free_space: &Arc<RwLock<FreeSpaceManager>>,
    entries: Vec<WriteEntry>,
    stats: &Arc<Statistics>,
    format: &dyn RecordFormat,
) -> Result<()> {
    let mut batch_writes = Vec::new();
    let mut delete_operations = Vec::new();
    let mut records_to_clear = Vec::new();

    for entry in entries {
        match entry.operation {
            Operation::Insert | Operation::Update => {
                if entry.record.refcount.load(Ordering::Acquire) > 0
                    && entry.record.sector.load(Ordering::Acquire) == 0
                {
                    let data = perpare_record_data(&entry.record, format)?;
                    let sectors_needed = data.len().div_ceil(BLOCK_SIZE);
                    let sector = free_space.write().allocate_sectors(sectors_needed as u64)?;

                    stats
                        .disk_usage
                        .fetch_add((sectors_needed * BLOCK_SIZE) as u64, Ordering::Relaxed);

                    batch_writes.push((sector, data));
                    records_to_clear.push((sector, entry.record.clone()));
                }
            }
            Operation::Delete => {
                let sector = entry.record.sector.load(Ordering::Acquire);
                if sector != 0 {
                    delete_operations.push((sector, entry.record.key.len(), entry.old_value_len));
                }
            }
            _ => {}
        }
    }

    for (sector, key_len, value_len) in delete_operations {
        let mut deletion_marker = vec![0u8; BLOCK_SIZE];
        deletion_marker[..8].copy_from_slice(DELETED_MARKER);

        let _ = disk_io.write().write_sectors_sync(sector, &deletion_marker);

        let total_size = SECTOR_HEADER_SIZE + 2 + key_len + 8 + 8 + value_len;
        let sectors_needed = total_size.div_ceil(BLOCK_SIZE);

        let _ = free_space
            .write()
            .release_sectors(sector, sectors_needed as u64);

        stats
            .disk_usage
            .fetch_sub((sectors_needed * BLOCK_SIZE) as u64, Ordering::Relaxed);
    }

    if !batch_writes.is_empty() {
        let mut retries = 3;
        let mut delay_us = 100;

        while retries > 0 {
            let result = disk_io.write().batch_write(batch_writes.clone());

            match result {
                Ok(()) => {
                    for (sector, record) in &records_to_clear {
                        record.sector.store(*sector, Ordering::Release);
                        std::sync::atomic::fence(Ordering::Release);
                        record.clear_value();
                    }
                    stats.record_write_flushed(records_to_clear.len() as u64);
                    break;
                }
                Err(e) => {
                    retries -= 1;
                    if retries > 0 {
                        let jitter = {
                            use rand::RngExt;
                            let mut rng = rand::rng();
                            (delay_us * rng.random_range(-10..=10)) / 100
                        };
                        let actual_delay = (delay_us + jitter).max(1);
                        thread::sleep(Duration::from_micros(actual_delay as u64));
                        delay_us *= 2;
                    } else {
                        stats.record_write_failed();
                        for (sector, _) in &records_to_clear {
                            free_space.write().release_sectors(*sector, 1)?;
                        }
                        return Err(e);
                    }
                }
            }
        }
    }

    Ok(())
}

fn perpare_record_data(record: &Record, format: &dyn RecordFormat) -> Result<Vec<u8>> {
    let total_size = format.total_size(record.key.len(), record.value_len);

    let sectors_needed = total_size.div_ceil(BLOCK_SIZE);
    let padded_size = sectors_needed * BLOCK_SIZE;

    let mut data = Vec::with_capacity(padded_size);

    data.extend_from_slice(&SECTOR_MARKER.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());

    let record_data = format.serialize_record(record, true);
    data.extend_from_slice(&record_data);

    data.resize(padded_size, 0);

    Ok(data)
}
