use crate::{core::FunKV, error::DbError};

#[test]
fn test_timestamp_conflict_resolution() {
    let db = FunKV::new(None).unwrap();

    let key = b"key";

    db.insert_with_timestamp(key, b"value1", Some(100)).unwrap();
    db.insert_with_timestamp(key, b"value2", Some(200)).unwrap();

    assert_eq!(db.get(key).unwrap(), b"value2");

    let result = db.insert_with_timestamp(key, b"value3", Some(150));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DbError::OlderTimestamp));

    assert_eq!(db.get(key).unwrap(), b"value2");
}

#[test]
fn test_delete_with_timestamp() {
    let db = FunKV::new(None).unwrap();

    let key = b"del_ts_key";

    db.insert_with_timestamp(key, b"value", Some(100)).unwrap();

    let result = db.delete_with_timestamp(key, Some(50));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DbError::OlderTimestamp));

    assert!(db.contains_key(key));

    db.delete_with_timestamp(key, Some(200)).unwrap();
    assert!(!db.contains_key(key));
}