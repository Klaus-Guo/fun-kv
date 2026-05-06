use std::{sync::{Arc, atomic::Ordering}, thread};

use fun_kv::stats::Statistics;

#[test]
fn test_statistics_creation() {
    let stats = Statistics::new();

    assert_eq!(stats.record_count.load(Ordering::Relaxed), 0);
    assert_eq!(stats.memory_usage.load(Ordering::Relaxed), 0);
    assert_eq!(stats.disk_usage.load(Ordering::Relaxed), 0);
    assert_eq!(stats.total_gets.load(Ordering::Relaxed), 0);
    assert_eq!(stats.total_inserts.load(Ordering::Relaxed), 0);
}

#[test]
fn test_record_operation() {
    let stats = Statistics::new();

    stats.record_get(100, true);
    stats.record_get(200, false);
    stats.record_insert(150, false);
    stats.record_insert(250, true);
    stats.record_delete(50);

    assert_eq!(stats.total_gets.load(Ordering::Relaxed), 2);
    assert_eq!(stats.total_inserts.load(Ordering::Relaxed), 1);
    assert_eq!(stats.total_updates.load(Ordering::Relaxed), 1);
    assert_eq!(stats.total_deletes.load(Ordering::Relaxed), 1);
    assert_eq!(stats.cache_hits.load(Ordering::Relaxed), 1);
    assert_eq!(stats.cache_misses.load(Ordering::Relaxed), 1);
}

#[test]
fn test_latency_recording() {
    let stats = Statistics::new();

    stats.record_get(1000, true);
    stats.record_get(2000, false);
    stats.record_range_query(3000);
    stats.record_insert(4000, false);
    stats.record_delete(5000);

    assert_eq!(stats.get_latency_ns.load(Ordering::Relaxed), 3000);
    assert_eq!(stats.range_query_latency_ns.load(Ordering::Relaxed), 3000);
    assert_eq!(stats.insert_latency_ns.load(Ordering::Relaxed), 4000);
    assert_eq!(stats.delete_latency_ns.load(Ordering::Relaxed), 5000);
}

#[test]
fn test_write_buffer_stats() {
    let stats = Statistics::new();

    stats.record_write_buffered();
    stats.record_write_buffered();
    stats.record_write_flushed(10);
    stats.record_write_failed();

    assert_eq!(stats.writes_buffered.load(Ordering::Relaxed), 2);
    assert_eq!(stats.writes_flushed.load(Ordering::Relaxed), 10);
    assert_eq!(stats.write_failures.load(Ordering::Relaxed), 1);
}

#[test]
fn test_eviction_stats() {
    let stats = Statistics::new();

    stats.record_eviction(5);
    stats.record_eviction(3);

    assert_eq!(stats.cache_evictions.load(Ordering::Relaxed), 8);
}

#[test]
fn test_statistics_snapshot() {
    let stats = Statistics::new();

    stats.record_count.store(100, Ordering::Relaxed);
    stats.memory_usage.store(10000, Ordering::Relaxed);
    stats.disk_bytes_written.store(50000, Ordering::Relaxed);
    stats.total_gets.store(500, Ordering::Relaxed);
    stats.total_inserts.store(100, Ordering::Relaxed);
    stats.cache_hits.store(400, Ordering::Relaxed);
    stats.cache_misses.store(100, Ordering::Relaxed);

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.record_count, 100);
    assert_eq!(snapshot.memory_usage, 10000);
    assert_eq!(snapshot.disk_bytes_written, 50000);
    assert_eq!(snapshot.total_gets, 500);
    assert_eq!(snapshot.total_inserts, 100);
    assert_eq!(snapshot.cache_hit_rate, 80.0);
}

#[test]
fn test_cache_hit_rate_calculation() {
    let stats = Statistics::new();

    let snapshot = stats.snapshot();
    assert_eq!(snapshot.cache_hit_rate, 0.0);

    stats.cache_hits.store(100, Ordering::Relaxed);
    stats.cache_misses.store(0, Ordering::Relaxed);
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.cache_hit_rate, 100.0);

    stats.cache_hits.store(50, Ordering::Relaxed);
    stats.cache_misses.store(50, Ordering::Relaxed);
    let snapshot = stats.snapshot();
    assert_eq!(snapshot.cache_hit_rate, 50.0);
}

#[test]
fn test_average_latencies() {
    let stats = Statistics::new();

    stats.total_gets.store(100, Ordering::Relaxed);
    stats.get_latency_ns.store(10000, Ordering::Relaxed);

    stats.total_range_queries.store(10, Ordering::Relaxed);
    stats.range_query_latency_ns.store(100000, Ordering::Relaxed);

    stats.total_inserts.store(50, Ordering::Relaxed);
    stats.insert_latency_ns.store(15000, Ordering::Relaxed);

    stats.total_deletes.store(25, Ordering::Relaxed);
    stats.delete_latency_ns.store(5000, Ordering::Relaxed);

    let snapshot = stats.snapshot();

    assert_eq!(snapshot.avg_get_latency_ns, 100);
    assert_eq!(snapshot.avg_range_query_latency_ns, 10000);
    assert_eq!(snapshot.avg_insert_latency_ns, 300);
    assert_eq!(snapshot.avg_delete_latency_ns, 200);
}

#[test]
fn test_concurrent_statistics() {
    let stats = Arc::new(Statistics::new());
    let mut handles = vec![];

    for _ in 0..10 {
        let stats_clone = Arc::clone(&stats);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                stats_clone.record_get(100, true);
                stats_clone.record_range_query(1000);
                stats_clone.record_insert(200, false);
                stats_clone.record_delete(50);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(stats.total_gets.load(Ordering::Relaxed), 1000);
    assert_eq!(stats.total_range_queries.load(Ordering::Relaxed), 1000);
    assert_eq!(stats.total_inserts.load(Ordering::Relaxed), 1000);
    assert_eq!(stats.total_deletes.load(Ordering::Relaxed), 1000);

    assert_eq!(stats.get_latency_ns.load(Ordering::Relaxed), 1000 * 100);
    assert_eq!(stats.range_query_latency_ns.load(Ordering::Relaxed), 1000 * 1000);
    assert_eq!(stats.insert_latency_ns.load(Ordering::Relaxed), 1000 * 200);
    assert_eq!(stats.delete_latency_ns.load(Ordering::Relaxed), 1000 * 50);
}

#[test]
fn test_memory_tracking() {
    let stats = Statistics::new();

    stats.memory_usage.store(1000, Ordering::Relaxed);
    assert_eq!(stats.memory_usage.load(Ordering::Relaxed), 1000);

    stats.memory_usage.fetch_add(500, Ordering::Relaxed);
    assert_eq!(stats.memory_usage.load(Ordering::Relaxed), 1500);

    stats.memory_usage.fetch_sub(200, Ordering::Relaxed);
    assert_eq!(stats.memory_usage.load(Ordering::Relaxed), 1300);
}

#[test]
fn test_flush_count() {
    let stats = Statistics::new();

    stats.flush_count.fetch_add(1, Ordering::Relaxed);
    stats.flush_count.fetch_add(1, Ordering::Relaxed);

    assert_eq!(stats.flush_count.load(Ordering::Relaxed), 2);
}