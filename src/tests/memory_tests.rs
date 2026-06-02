use crate::{DbBuilder, core::FunKV, error::DbError};

#[test]
fn test_memory_limit_enforcement() {
    let db = DbBuilder::new()
        .max_memory(1024)
        .build()
        .unwrap();

    db.insert(b"k1", b"v1").unwrap();

    let large_value = vec![0u8; 2048];
    let result = db.insert(b"k2", &large_value);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DbError::OutOfMemory));
}

#[test]
fn test_memory_usage_tracking() {
    let db = FunKV::new(None).unwrap();

    let initial_usage = db.memory_usage();

    for i in 0..10 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i);
        db.insert(key.as_bytes(), value.as_bytes()).unwrap();
    }

    let after_insert_usage = db.memory_usage();
    assert!(after_insert_usage > initial_usage);
    assert_eq!(after_insert_usage - initial_usage, 1320);

    for i in 0..5 {
        let key = format!("key_{}", i);
        db.delete(key.as_bytes()).unwrap();
    }

    let after_delete_usage = db.memory_usage();
    assert!(after_delete_usage < after_insert_usage);
    assert_eq!(after_insert_usage - after_delete_usage, 660);
}