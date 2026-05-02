use std::{
    mem,
    sync::{
        Arc,
        atomic::{self, AtomicBool, AtomicU32, AtomicUsize, Ordering},
    },
};

use bytes::Bytes;
use parking_lot::{Mutex, RwLock};

use crate::{constants::*, stats::Statistics, utils::hash::murmur3_32};

pub struct Cache {
    buckets: Vec<RwLock<Vec<CacheEntity>>>,

    clock_hand: AtomicUsize,

    high_watermark: AtomicUsize,

    low_watermark: AtomicUsize,

    eviction_lock: Mutex<()>,

    stats: Arc<Statistics>,
}

#[derive(Clone)]
struct CacheEntity {
    key: Vec<u8>,
    value: Bytes,

    reference_bit: Arc<AtomicBool>,

    size: usize,

    access_count: Arc<AtomicU32>,
}

impl Cache {
    pub fn new(stats: Arc<Statistics>) -> Self {
        let buckets = (0..CACHE_BUCKETS)
            .map(|_| RwLock::new(Vec::new()))
            .collect();

        Self {
            buckets,
            clock_hand: AtomicUsize::new(0),
            high_watermark: AtomicUsize::new(CACHE_HIGH_WATERMARK_MB * MB),
            low_watermark: AtomicUsize::new(CACHE_LOW_WATERMARK_MB * MB),
            eviction_lock: Mutex::new(()),
            stats,
        }
    }

    pub fn get(&self, key: &[u8]) -> Option<Bytes> {
        let bucket_idx = Self::get_bucket_idx(key);

        let bucket = self.buckets[bucket_idx].read();

        for entry in bucket.iter() {
            if entry.key == key {
                entry.reference_bit.store(true, Ordering::Release);
                entry.access_count.fetch_add(1, Ordering::Relaxed);
                return Some(entry.value.clone());
            }
        }

        None
    }

    pub fn insert(&self, key: Vec<u8>, value: Bytes) {
        let size = key.len() + value.len() + mem::size_of::<CacheEntity>();

        // will not cache too large object
        let high_watermark = self.high_watermark.load(Ordering::Acquire);
        if size > high_watermark / 4 {
            return;
        }

        let current_usage = self.stats.cache_memory.load(Ordering::Acquire);
        let high_watermark = self.high_watermark.load(Ordering::Acquire);
        if current_usage + size > high_watermark {
            self.evict_entries();
        }

        let bucket_idx = Self::get_bucket_idx(&key);

        let mut bucket = self.buckets[bucket_idx].write();

        for entry in bucket.iter_mut() {
            if entry.key == key {
                let old_size = entry.size;
                entry.value = value;
                entry.size = size;
                entry.reference_bit.store(true, Ordering::Release);

                if size > old_size {
                    self.stats
                        .cache_memory
                        .fetch_add(size - old_size, Ordering::AcqRel);
                } else {
                    self.stats
                        .cache_memory
                        .fetch_sub(old_size - size, Ordering::AcqRel);
                }

                return;
            }
        }

        let entry = CacheEntity {
            key,
            value,
            reference_bit: Arc::new(AtomicBool::new(true)),
            size,
            access_count: Arc::new(AtomicU32::new(1)),
        };

        bucket.push(entry);
        self.stats.cache_memory.fetch_add(size, Ordering::AcqRel);
    }

    pub fn remove(&self, key: &[u8]) {
        let bucket_idx = Self::get_bucket_idx(key);

        let mut bucket = self.buckets[bucket_idx].write();

        if let Some(pos) = bucket.iter().position(|e| e.key == key) {
            let entry = bucket.remove(pos);
            self.stats
                .cache_memory
                .fetch_sub(entry.size, Ordering::AcqRel);
        }
    }

    // Clock eviction
    pub fn evict_entries(&self) {
        let _lock = match self.eviction_lock.try_lock() {
            Some(lock) => lock,
            None => return,
        };

        let target_usage = self.low_watermark.load(Ordering::Acquire);
        let mut current_usage = self.stats.cache_memory.load(Ordering::Acquire);

        if current_usage <= target_usage {
            return;
        }

        let mut scans = 0;

        while current_usage > target_usage && scans < MAX_SCANS {
            let mut entries_checked = 0;
            let mut bucket_count = 0;

            for bucket in &self.buckets {
                bucket_count += bucket.read().len();
            }

            let total_entries = bucket_count;

            while entries_checked < total_entries && current_usage > target_usage {
                let hand = self.clock_hand.fetch_add(1, Ordering::AcqRel) % CACHE_BUCKETS;
                let mut bucket = self.buckets[hand].write();

                let mut i = 0;
                while i < bucket.len() {
                    let entry = &bucket[i];

                    if entry.reference_bit.load(Ordering::Acquire) {
                        entry.reference_bit.store(false, Ordering::Release);
                        atomic::fence(Ordering::Release);

                        i += 1;
                    } else {
                        let removed = bucket.remove(i);
                        self.stats
                            .cache_memory
                            .fetch_sub(removed.size, Ordering::AcqRel);
                        self.stats.record_eviction(1);
                        current_usage -= removed.size;
                    }

                    entries_checked += 1;

                    if current_usage <= target_usage {
                        break;
                    }
                }
            }

            scans += 1;
        }
    }

    pub fn clear(&self) {
        for bucket in &self.buckets {
            bucket.write().clear();
        }

        self.stats.cache_memory.store(0, Ordering::Release);
        self.clock_hand.store(0, Ordering::Release);
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            memory_usage: self.stats.cache_memory.load(Ordering::Acquire),
            high_watermark: self.high_watermark.load(Ordering::Acquire),
            low_watermark: self.low_watermark.load(Ordering::Acquire),
        }
    }

    pub fn adjust_watermarks(&self, high_mb: usize, low_mb: usize) {
        let high = high_mb * MB;
        let low = low_mb * MB;

        if high > low && high <= CACHE_MAX_SIZE {
            self.high_watermark.store(high, Ordering::Release);
            self.low_watermark.store(low, Ordering::Release);

            let current_usage = self.stats.cache_memory.load(Ordering::Acquire);

            if current_usage > high {
                self.evict_entries();
            }
        }
    }

    fn get_bucket_idx(key: &[u8]) -> usize {
        let hash = murmur3_32(key, 0);

        (hash as usize) % CACHE_BUCKETS
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub memory_usage: usize,
    pub high_watermark: usize,
    pub low_watermark: usize,
}
