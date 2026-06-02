use std::sync::Arc;

use crate::{constants::*, core::cache::Cache, stats::Statistics};

#[test]
fn test_basic_cache_operations() {
    let stats = Arc::new(Statistics::new());
    let cache = Cache::new(stats);

    let key = b"test_key".to_vec();
    let value = b"test_value".to_vec();

    // Insert and retrieve
    cache.insert(key.clone(), value.clone());
    let retrieved = cache.get(&key).unwrap();
    assert_eq!(retrieved, value);

    // Remove and verify gone
    cache.remove(&key);
    assert!(cache.get(&key).is_none());
}

#[test]
fn test_cache_eviction() {
    let stats = Arc::new(Statistics::new());
    let cache = Cache::new(stats.clone());

    // Set lower watermarks for testing
    cache.adjust_watermarks(1, 0); // 1MB high, 0 low

    // Insert many entries to trigger eviction
    for i in 0..10000 {
        let key = format!("key_{}", i).into_bytes();
        let value = vec![0u8; KB]; // 1KB each
        cache.insert(key, value);
    }

    let cache_stats = cache.stats();
    assert!(cache_stats.memory_usage <= cache_stats.high_watermark);
}

#[test]
fn test_reference_bit_behavior() {
    let stats = Arc::new(Statistics::new());
    let cache = Cache::new(stats);

    // Insert entries
    for i in 0..100 {
        let key = format!("key_{}", i).into_bytes();
        let value = format!("value_{}", i).into_bytes();
        cache.insert(key, value);
    }

    // Access some entries to set reference bits
    for i in 0..50 {
        let key = format!("key_{}", i).into_bytes();
        cache.get(&key);
    }

    // These entries should be less likely to be evicted
    // due to their reference bits being set
}

#[test]
fn test_cache_clear() {
    let stats = Arc::new(Statistics::new());
    let cache = Cache::new(stats.clone());

    // Insert some entries
    for i in 0..10 {
        let key = format!("key_{}", i).into_bytes();
        let value = format!("value_{}", i).into_bytes();
        cache.insert(key, value);
    }

    // Clear cache
    cache.clear();

    // Verify all entries are gone
    for i in 0..10 {
        let key = format!("key_{}", i).into_bytes();
        assert!(cache.get(&key).is_none());
    }

    // Verify memory usage is 0
    assert_eq!(cache.stats().memory_usage, 0);
}

#[test]
fn test_cache_update_existing() {
    let stats = Arc::new(Statistics::new());
    let cache = Cache::new(stats);

    let key = b"test_key".to_vec();
    let value1 = b"value1".to_vec();
    let value2 = b"much_longer_value2".to_vec();

    // Insert initial value
    cache.insert(key.clone(), value1.clone());
    assert_eq!(cache.get(&key).unwrap(), value1);

    // Update with new value
    cache.insert(key.clone(), value2.clone());
    assert_eq!(cache.get(&key).unwrap(), value2);
}

#[test]
fn test_cache_large_value_rejection() {
    let stats = Arc::new(Statistics::new());
    let cache = Cache::new(stats.clone());

    // Try to insert a value larger than 1/4 of high watermark
    let key = b"huge_key".to_vec();
    let huge_value = vec![0u8; CACHE_HIGH_WATERMARK_MB * MB / 3];

    cache.insert(key.clone(), huge_value);

    // Should not be cached
    assert!(cache.get(&key).is_none());
}

#[test]
fn test_cache_watermark_adjustment() {
    let stats = Arc::new(Statistics::new());
    let cache = Cache::new(stats.clone());

    // Adjust watermarks
    cache.adjust_watermarks(10, 5); // 10MB high, 5MB low

    let cache_stats = cache.stats();
    assert_eq!(cache_stats.high_watermark, 10 * MB);
    assert_eq!(cache_stats.low_watermark, 5 * MB);

    // Invalid adjustment should be ignored
    cache.adjust_watermarks(5, 10); // Invalid: low > high
    let cache_stats = cache.stats();
    assert_eq!(cache_stats.high_watermark, 10 * MB); // Unchanged
}

#[test]
fn test_concurrent_cache_access() {
    use std::thread;

    let stats = Arc::new(Statistics::new());
    let cache = Arc::new(Cache::new(stats));

    let mut handles = vec![];

    // Multiple threads inserting
    for i in 0..10 {
        let cache_clone = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let key = format!("thread{}:key{}", i, j).into_bytes();
                let value = format!("value_{}_{}", i, j).into_bytes();
                cache_clone.insert(key, value);
            }
        }));
    }

    // Multiple threads reading
    for i in 0..10 {
        let cache_clone = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let key = format!("thread{}:key{}", i, j).into_bytes();
                cache_clone.get(&key);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}