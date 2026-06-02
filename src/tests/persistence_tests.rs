use std::{thread, time::Duration};

use tempfile::NamedTempFile;

use crate::core::FunKV;

#[test]
fn test_basic_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    // Create store and insert data
    {
        let db = FunKV::new(Some(path.clone())).unwrap();

        db.insert(b"persist_key", b"persist_value").unwrap();
        db.insert(b"another_key", b"another_value").unwrap();

        db.flush();
    } // Store is dropped here

    // Reopen and verify data persisted
    {
        let db = FunKV::new(Some(path)).unwrap();

        let value = db.get(b"persist_key").unwrap();
        assert_eq!(value, b"persist_value");

        let value2 = db.get(b"another_key").unwrap();
        assert_eq!(value2, b"another_value");
    }
}

#[test]
fn test_flush_all() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    let db = FunKV::new(Some(path)).unwrap();

    // Insert data
    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i);
        db.insert(key.as_bytes(), value.as_bytes()).unwrap();
    }

    // Force flush
    db.flush_all();

    // Data should be on disk even without dropping store
    assert_eq!(db.len(), 100);
}

#[test]
fn test_graceful_shutdown() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    // Insert data and let Drop handle flushing
    {
        let db = FunKV::new(Some(path.clone())).unwrap();

        for i in 0..20 {
            let key = format!("shutdown_key_{}", i);
            let value = format!("shutdown_value_{}", i);
            db.insert(key.as_bytes(), value.as_bytes()).unwrap();
        }

        // Store drops here - Drop impl will flush
    }

    // Reopen and verify all data persisted (thanks to Drop)
    {
        let db = FunKV::new(Some(path)).unwrap();

        // All keys should be present due to graceful shutdown
        for i in 0..20 {
            let key = format!("shutdown_key_{}", i);
            assert!(db.contains_key(key.as_bytes()));
        }
    }
}

#[test]
fn test_value_offloading() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    let db = FunKV::new(Some(path)).unwrap();

    // Insert large value
    let large_value = vec![0xAB; 100_000]; // 100KB
    db.insert(b"large_key", &large_value).unwrap();

    // Force flush to disk
    db.flush();

    // Wait for write buffer to process
    thread::sleep(Duration::from_millis(100));

    // Value should still be retrievable
    let retrieved = db.get(b"large_key").unwrap();
    assert_eq!(retrieved, large_value);
}

#[test]
fn test_metadata_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    let initial_count;

    // Create store and get initial stats
    {
        let db = FunKV::new(Some(path.clone())).unwrap();

        for i in 0..25 {
            let key = format!("meta_key_{}", i);
            db.insert(key.as_bytes(), b"value").unwrap();
        }

        initial_count = db.len();
        db.flush_all();
    }

    // Reopen and verify metadata
    {
        let db = FunKV::new(Some(path)).unwrap();
        assert_eq!(db.len(), initial_count);
    }
}

#[test]
fn test_concurrent_persistence() {
    use std::sync::Arc;

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    let db = Arc::new(FunKV::new(Some(path.clone())).unwrap());
    let mut handles = vec![];

    // Multiple threads writing
    for t in 0..5 {
        let store_clone = Arc::clone(&db);
        handles.push(thread::spawn(move || {
            for i in 0..20 {
                let key = format!("thread{}:key{}", t, i);
                let value = format!("value_{}_{}", t, i);
                store_clone
                    .insert(key.as_bytes(), value.as_bytes())
                    .unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    db.flush_all();
    drop(db);

    // Verify all data persisted
    let db = FunKV::new(Some(path)).unwrap();
    assert_eq!(db.len(), 100); // 5 threads * 20 keys
}

#[test]
fn test_delete_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    // Insert and delete
    {
        let db = FunKV::new(Some(path.clone())).unwrap();

        db.insert(b"del_key1", b"value1").unwrap();
        db.insert(b"del_key2", b"value2").unwrap();
        db.insert(b"keep_key", b"keep_value").unwrap();

        db.delete(b"del_key1").unwrap();
        db.delete(b"del_key2").unwrap();

        db.flush_all();
    }

    // Verify deletes persisted
    {
        let db = FunKV::new(Some(path)).unwrap();

        assert!(!db.contains_key(b"del_key1"));
        assert!(!db.contains_key(b"del_key2"));
        assert!(db.contains_key(b"keep_key"));
        assert_eq!(db.len(), 1);
    }
}

#[test]
fn test_update_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    // Multiple updates
    {
        let db = FunKV::new(Some(path.clone())).unwrap();

        db.insert(b"update_key", b"value1").unwrap();
        db.insert(b"update_key", b"value2").unwrap();
        db.insert(b"update_key", b"value3").unwrap();
        db.insert(b"update_key", b"final_value").unwrap();

        db.flush_all();
    }

    // Verify only latest value persisted
    {
        let db = FunKV::new(Some(path)).unwrap();

        let value = db.get(b"update_key").unwrap();
        assert_eq!(value, b"final_value");
    }
}

#[test]
fn test_atomic_increment_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    // Perform atomic operations
    {
        let db = FunKV::new(Some(path.clone())).unwrap();

        let zero: i64 = 0;
        db.insert(b"counter", &zero.to_le_bytes()).unwrap();

        for _ in 0..100 {
            db.atomic_increment(b"counter", 1).unwrap();
        }

        db.flush_all();
    }

    // Verify counter value persisted
    {
        let db = FunKV::new(Some(path)).unwrap();

        let value = db.atomic_increment(b"counter", 0).unwrap();
        assert_eq!(value, 100);
    }
}

#[test]
fn test_range_query_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();

    // Insert sorted data
    {
        let db = FunKV::new(Some(path.clone())).unwrap();

        for i in 0..50 {
            let key = format!("item:{:03}", i);
            let value = format!("value_{}", i);
            db.insert(key.as_bytes(), value.as_bytes()).unwrap();
        }

        db.flush_all();
    }

    // Verify range queries work after restart
    {
        let db = FunKV::new(Some(path)).unwrap();

        let results = db.range_query(b"item:010", b"item:020", 100).unwrap();
        assert_eq!(results.len(), 11); // 010 through 020 inclusive

        assert_eq!(results[0].0, b"item:010");
        assert_eq!(results[10].0, b"item:020");
    }
}
