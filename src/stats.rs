use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::error::DbError;

#[derive(Debug)]
pub struct Statistics {
    pub record_count: AtomicU32,
    pub memory_usage: AtomicUsize,
    pub disk_usage: AtomicU64,

    pub total_gets: AtomicU64,
    pub total_inserts: AtomicU64,
    pub total_updates: AtomicU64,
    pub total_deletes: AtomicU64,
    pub total_range_queries: AtomicU64,

    pub get_latency_ns: AtomicU64,
    pub insert_latency_ns: AtomicU64,
    pub delete_latency_ns: AtomicU64,

    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub cache_evictions: AtomicU64,
    pub cache_memory: AtomicUsize,

    pub writes_buffered: AtomicU64,
    pub writes_flushed: AtomicU64,
    pub write_failures: AtomicU64,
    pub flush_count: AtomicU64,

    pub disk_reads: AtomicU64,
    pub disk_writes: AtomicU64,
    pub disk_bytes_read: AtomicU64,
    pub disk_bytes_written: AtomicU64,

    pub key_not_found_errors: AtomicU64,
    pub out_of_memory_errors: AtomicU64,
    pub io_errors: AtomicU64,

    pub ttl_expired_lazy: AtomicU64,
    pub ttl_expired_active: AtomicU64,
    pub ttl_cleaner_runs: AtomicU64,
    pub keys_with_ttl: AtomicU64,
}

impl Statistics {
    pub fn new() -> Self {
        Self {
            record_count: AtomicU32::new(0),
            memory_usage: AtomicUsize::new(0),
            disk_usage: AtomicU64::new(0),
            total_gets: AtomicU64::new(0),
            total_inserts: AtomicU64::new(0),
            total_updates: AtomicU64::new(0),
            total_deletes: AtomicU64::new(0),
            total_range_queries: AtomicU64::new(0),
            get_latency_ns: AtomicU64::new(0),
            insert_latency_ns: AtomicU64::new(0),
            delete_latency_ns: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            cache_evictions: AtomicU64::new(0),
            cache_memory: AtomicUsize::new(0),
            writes_buffered: AtomicU64::new(0),
            writes_flushed: AtomicU64::new(0),
            write_failures: AtomicU64::new(0),
            flush_count: AtomicU64::new(0),
            disk_reads: AtomicU64::new(0),
            disk_writes: AtomicU64::new(0),
            disk_bytes_read: AtomicU64::new(0),
            disk_bytes_written: AtomicU64::new(0),
            key_not_found_errors: AtomicU64::new(0),
            out_of_memory_errors: AtomicU64::new(0),
            io_errors: AtomicU64::new(0),
            ttl_expired_lazy: AtomicU64::new(0),
            ttl_expired_active: AtomicU64::new(0),
            ttl_cleaner_runs: AtomicU64::new(0),
            keys_with_ttl: AtomicU64::new(0),
        }
    }

    pub fn record_get(&self, latency_ns: u64, hit: bool) {
        self.total_gets.fetch_add(1, Ordering::Relaxed);
        self.get_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);

        if hit {
            self.cache_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.cache_misses.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn record_insert(&self, latency_ns: u64, is_update: bool) {
        if is_update {
            self.total_updates.fetch_add(1, Ordering::Relaxed);
        } else {
            self.total_inserts.fetch_add(1, Ordering::Relaxed);
        }
        self.insert_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);
    }

    pub fn record_delete(&self, latency_ns: u64) {
        self.total_deletes.fetch_add(1, Ordering::Relaxed);
        self.delete_latency_ns
            .fetch_add(latency_ns, Ordering::Relaxed);
    }

    pub fn record_range_query(&self) {
        self.total_range_queries.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_eviction(&self, count: u64) {
        self.cache_evictions.fetch_add(count, Ordering::Relaxed);
    }

    pub fn record_write_buffered(&self) {
        self.writes_buffered.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_write_flushed(&self, count: u64) {
        self.writes_flushed.fetch_add(count, Ordering::Relaxed);
    }

    pub fn record_write_failed(&self) {
        self.write_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_disk_read(&self, bytes: u64) {
        self.disk_reads.fetch_add(1, Ordering::Relaxed);
        self.disk_bytes_read.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_disk_write(&self, bytes: u64) {
        self.disk_writes.fetch_add(1, Ordering::Relaxed);
        self.disk_bytes_written.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_error(&self, error: &DbError) {
        match error {
            DbError::KeyNotFound => {
                self.key_not_found_errors.fetch_add(1, Ordering::Relaxed);
            }
            DbError::OutOfMemory => {
                self.out_of_memory_errors.fetch_add(1, Ordering::Relaxed);
            }
            DbError::IoError(_) => {
                self.io_errors.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        let total_ops = self.total_gets.load(Ordering::Relaxed)
            + self.total_inserts.load(Ordering::Release)
            + self.total_updates.load(Ordering::Relaxed)
            + self.total_deletes.load(Ordering::Relaxed);

        let avg_get_latency = {
            let gets = self.total_gets.load(Ordering::Relaxed);
            if gets > 0 {
                self.get_latency_ns.load(Ordering::Relaxed) / gets
            } else {
                0
            }
        };

        let avg_insert_latency = {
            let inserts = self.total_inserts.load(Ordering::Relaxed)
                + self.total_updates.load(Ordering::Relaxed);
            if inserts > 0 {
                self.insert_latency_ns.load(Ordering::Relaxed) / inserts
            } else {
                0
            }
        };

        let avg_delete_latency = {
            let deletes = self.total_deletes.load(Ordering::Relaxed);
            if deletes > 0 {
                self.delete_latency_ns.load(Ordering::Relaxed) / deletes
            } else {
                0
            }
        };

        let cache_hit_rate = {
            let cache_hits = self.cache_hits.load(Ordering::Relaxed);
            let total_cache_ops = cache_hits + self.cache_misses.load(Ordering::Relaxed);

            if total_cache_ops > 0 {
                (cache_hits as f64 / total_cache_ops as f64) * 100.0
            } else {
                0.0
            }
        };

        StatsSnapshot {
            record_count: self.record_count.load(Ordering::Relaxed),
            memory_usage: self.memory_usage.load(Ordering::Relaxed),
            total_operations: total_ops,
            total_gets: self.total_gets.load(Ordering::Relaxed),
            total_inserts: self.total_inserts.load(Ordering::Relaxed),
            total_updates: self.total_updates.load(Ordering::Relaxed),
            total_deletes: self.total_deletes.load(Ordering::Relaxed),
            total_range_queries: self.total_range_queries.load(Ordering::Relaxed),
            avg_get_latency_ns: avg_get_latency,
            avg_insert_latency_ns: avg_insert_latency,
            avg_delete_latency_ns: avg_delete_latency,
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            cache_hit_rate,
            cache_evictions: self.cache_evictions.load(Ordering::Relaxed),
            cache_memory: self.cache_memory.load(Ordering::Relaxed),
            writes_buffered: self.writes_buffered.load(Ordering::Relaxed),
            writes_flushed: self.writes_flushed.load(Ordering::Relaxed),
            write_failures: self.write_failures.load(Ordering::Relaxed),
            flush_count: self.flush_count.load(Ordering::Relaxed),
            disk_reads: self.disk_reads.load(Ordering::Relaxed),
            disk_writes: self.disk_writes.load(Ordering::Relaxed),
            disk_bytes_read: self.disk_bytes_read.load(Ordering::Relaxed),
            disk_bytes_written: self.disk_bytes_written.load(Ordering::Relaxed),
            key_not_found_errors: self.key_not_found_errors.load(Ordering::Relaxed),
            out_of_memory_errors: self.out_of_memory_errors.load(Ordering::Relaxed),
            io_errors: self.io_errors.load(Ordering::Relaxed),
        }
    }
}

impl Default for Statistics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct StatsSnapshot {
    pub record_count: u32,
    pub memory_usage: usize,

    pub total_operations: u64,
    pub total_gets: u64,
    pub total_inserts: u64,
    pub total_updates: u64,
    pub total_deletes: u64,
    pub total_range_queries: u64,

    pub avg_get_latency_ns: u64,
    pub avg_insert_latency_ns: u64,
    pub avg_delete_latency_ns: u64,

    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,
    pub cache_evictions: u64,
    pub cache_memory: usize,

    pub writes_buffered: u64,
    pub writes_flushed: u64,
    pub write_failures: u64,
    pub flush_count: u64,

    pub disk_reads: u64,
    pub disk_writes: u64,
    pub disk_bytes_read: u64,
    pub disk_bytes_written: u64,

    pub key_not_found_errors: u64,
    pub out_of_memory_errors: u64,
    pub io_errors: u64,
}

impl StatsSnapshot {
    pub fn format(&self) -> String {
        format!(
            "=== DB Statistics ===\n\
            Store:\n\
            - Records: {}\n\
            - Memory: {:.2} MB\n\n\
            Operations:\n\
            - Total: {}\n\
            - Gets: {} (avg latency: {:.2}μs)\n\
            - Inserts: {} (avg latency: {:.2}μs)\n\
            - Updates: {}\n\
            - Deletes: {} (avg latency: {:.2}μs)\n\
            - Range Queries: {}\n\n\
            Cache:\n\
            - Hit Rate: {:.1}%\n\
            - Hits: {}\n\
            - Misses: {}\n\
            - Evictions: {}\n\
            - Memory: {:.2} MB\n\n\
            Write Buffer:\n\
            - Buffered: {}\n\
            - Flushed: {}\n\
            - Failures: {}\n\
            - Flush Count: {}\n\n\
            Disk I/O:\n\
            - Reads: {} ({:.2} MB)\n\
            - Writes: {} ({:.2} MB)\n\n\
            Errors:\n\
            - Key Not Found: {}\n\
            - Out of Memory: {}\n\
            - I/O Errors: {}",
            self.record_count,
            self.memory_usage as f64 / 1_048_576.0,
            self.total_operations,
            self.total_gets,
            self.avg_get_latency_ns as f64 / 1000.0,
            self.total_inserts,
            self.avg_insert_latency_ns as f64 / 1000.0,
            self.total_updates,
            self.total_deletes,
            self.avg_delete_latency_ns as f64 / 1000.0,
            self.total_range_queries,
            self.cache_hit_rate,
            self.cache_hits,
            self.cache_misses,
            self.cache_evictions,
            self.cache_memory as f64 / 1_048_576.0,
            self.writes_buffered,
            self.writes_flushed,
            self.write_failures,
            self.flush_count,
            self.disk_reads,
            self.disk_bytes_read as f64 / 1_048_576.0,
            self.disk_writes,
            self.disk_bytes_written as f64 / 1_048_576.0,
            self.key_not_found_errors,
            self.out_of_memory_errors,
            self.io_errors
        )
    }
}
