use std::{sync::{Arc, Weak, atomic::{AtomicBool, AtomicU64, Ordering}}, thread::{self, JoinHandle}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};

use ahash::RandomState;
use rand::{RngExt, rngs::ThreadRng};
use scc::HashMap;

use crate::{constants::Operation, core::{FunKV, record::Record}};

#[derive(Clone, Debug)]
pub struct TtlConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub max_time_per_run: Duration,
    pub max_iterations: usize,
    pub expiry_threshold: f32,
    pub sample_size: usize,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: Duration::from_millis(1000),
            max_time_per_run: Duration::from_millis(1),
            max_iterations: 16,
            expiry_threshold: 0.25,
            sample_size: 100,
        }
    }
}

impl TtlConfig {
    pub fn default() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }
}

pub struct TtlSweeper {
    store: Weak<FunKV>,
    config: TtlConfig,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    stats: TtlSweeperStats,
}

impl TtlSweeper {
    pub fn new(store: Weak<FunKV>, config: TtlConfig) -> Self {
        Self {
            store,
            config,
            shutdown: Arc::new(AtomicBool::new(false)),
            handle: None,
            stats: TtlSweeperStats::new(),
        }
    }

    pub fn start(&mut self) {
        if !self.config.enabled {
            return;
        }

        let store = self.store.clone();
        let config = self.config.clone();
        let shutdown = self.shutdown.clone();
        let stats = TtlSweeperStats {
            total_sampled: self.stats.total_sampled.clone(),
            total_expired: self.stats.total_expired.clone(),
            total_runs: self.stats.total_runs.clone(),
            last_run: self.stats.last_run.clone(),
        };

        let handle = thread::spawn(move || {
            run_sweeper_loop(store, config, shutdown, stats);
        });

        self.handle = Some(handle);
    }

    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Release);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    pub fn stats(&self) -> TtlSweeperSnapshot {
        TtlSweeperSnapshot {
            total_sampled: self.stats.total_sampled.load(Ordering::Relaxed),
            total_expired: self.stats.total_expired.load(Ordering::Relaxed),
            total_runs: self.stats.total_runs.load(Ordering::Relaxed),
            last_run: self.stats.last_run.load(Ordering::Relaxed),
        }
    }
}

impl Drop for TtlSweeper {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Clone)]
pub struct TtlSweeperSnapshot {
    pub total_sampled: u64,
    pub total_expired: u64,
    pub total_runs: u64,
    pub last_run: u64,
}


fn run_sweeper_loop(store: Weak<FunKV>, config: TtlConfig, shutdown: Arc<AtomicBool>, stats: TtlSweeperStats) {
    while !shutdown.load(Ordering::Acquire) {
        thread::sleep(config.interval);

        let Some(store) = store.upgrade() else {
            break;
        };

        let start = Instant::now();
        let mut iterations = 0;
        let mut total_sampled = 0;
        let mut total_expired = 0;

        loop {
            let (sampled, expired) = sample_and_expire_batch(&store, &config);
            total_sampled += sampled;
            total_expired += expired;
            iterations += 1;

            let expiry_rate = if sampled > 0 {
                expired as f32 / sampled as f32
            } else {
                0.0
            };

            if expiry_rate < config.expiry_threshold {
                break;
            }

            if iterations >= config.max_iterations {
                break;
            }
            if start.elapsed() > config.max_time_per_run {
                break;
            }
        }

        if total_sampled > 0 {
            stats.total_sampled.fetch_add(total_sampled, Ordering::Relaxed);
            stats.total_expired.fetch_add(total_expired, Ordering::Relaxed);
            stats.total_runs.fetch_add(1, Ordering::Relaxed);
            stats.last_run.store(
                SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64, 
            Ordering::Relaxed);
        }

        if shutdown.load(Ordering::Acquire) {
            break;
        }
    }
}

fn sample_and_expire_batch(store: &Arc<FunKV>, config: &TtlConfig) -> (u64, u64) {
    let now = store.get_timestamp();
    let mut sampled = 0;
    let mut expired = 0;
    let mut rng = rand::rng();

    let hash_table = store.get_hash_table();

    for _ in 0..config.sample_size {
        if let Some((key, record)) = get_random_ttl_entry(hash_table, &mut rng) {
            sampled += 1;
            
            let ttl = record.ttl.load(Ordering::Relaxed);
            
            if ttl > 0 && ttl < now {
                hash_table.remove_sync(&key);
                store.remove_from_tree(&key);

                expired += 1;

                if record.sector.load(Ordering::Relaxed) > 0 {
                    if let Some(wb) = store.get_write_buffer() {
                        let _ = wb.add_write(Operation::Delete, record, 0);
                    }
                }
            }
        }
    }

    (sampled, expired)
}

fn get_random_ttl_entry(hash_table: &HashMap<Vec<u8>, Arc<Record>, RandomState>, rng: &mut ThreadRng) -> Option<(Vec<u8>, Arc<Record>)> {
    let mut candidates = Vec::new();
    let mut count = 0;

    hash_table.iter_sync(|key: &Vec<u8>, value: &Arc<Record>| {
        if count >= 100 {
            return true;
        }
        count += 1;

        if value.ttl.load(Ordering::Relaxed) > 0 {
            candidates.push((key.clone(), value.clone()));
        }

        true
    });

    if candidates.is_empty() {
        None
    } else {
        let idx = rng.random_range(0..candidates.len());
        Some(candidates.into_iter().nth(idx).unwrap())
    }
}

pub struct TtlSweeperStats {
    pub total_sampled: Arc<AtomicU64>,
    pub total_expired: Arc<AtomicU64>,
    pub total_runs: Arc<AtomicU64>,
    pub last_run: Arc<AtomicU64>,
}

impl TtlSweeperStats {
    fn new() -> Self {
        Self {
            total_sampled: Arc::new(AtomicU64::new(0)),
            total_expired: Arc::new(AtomicU64::new(0)),
            total_runs: Arc::new(AtomicU64::new(0)),
            last_run: Arc::new(AtomicU64::new(0)),
        }
    }
}