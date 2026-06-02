use crate::{constants::MAX_KEY_SIZE, core::FunKV, error::DbError};

#[test]
fn test_empty_key_rejected() {
    let db = FunKV::new(None).unwrap();
    let result = db.insert(b"", b"value");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DbError::InvalidKeySize));
}

#[test]
fn test_empty_value_rejected() {
    let db = FunKV::new(None).unwrap();
    let result = db.insert(b"key", b"");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DbError::InvalidValueSize));
}

#[test]
fn test_max_key_size() {
    let db = FunKV::new(None).unwrap();
    
    let large_key = vec![b'k'; 100000];
    db.insert(&large_key, b"value").unwrap();
    assert_eq!(db.get(&large_key).unwrap(), b"value");

    let oversized_key = vec![b'k'; MAX_KEY_SIZE + 10];
    let result = db.insert(&oversized_key, b"value");
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DbError::InvalidKeySize));
}

#[test]
fn test_stats_snapshot() {
    let db = FunKV::new(None).unwrap();

    for i in 0..10 {
        let key = format!("key_{}", i);
        db.insert(key.as_bytes(), b"value").unwrap();
    }

    let stats = db.stats();
    assert_eq!(stats.record_count, 10);
    assert!(stats.memory_usage > 0);
    assert_eq!(stats.total_inserts, 10);
}