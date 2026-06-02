use crate::{core::FunKV, error::DbError};

#[test]
fn test_basic_operations() {
    let db = FunKV::new(None).unwrap();

    let key = b"test_key";
    let value = b"test_value";

    db.insert(key, value).unwrap();

    let retrieved = db.get(key).unwrap();
    assert_eq!(retrieved.as_slice(), value);

    db.delete(key).unwrap();

    let result = db.get(key);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DbError::KeyNotFound));
}

#[test]
fn test_update_existing_key() {
    let db = FunKV::new(None).unwrap();

    let key = b"key";
    let value = b"value";
    let updated_value = b"new_value";

    db.insert(key, value).unwrap();
    assert_eq!(db.get(key).unwrap(), value);

    db.insert(key, updated_value).unwrap();
    assert_eq!(db.get(key).unwrap(), updated_value);
}

#[test]
fn test_empty_db() {
    let db = FunKV::new(None).unwrap();

    assert!(db.is_empty());
    assert_eq!(db.len(), 0);
}

#[test]
fn test_contains_key() {
    let db = FunKV::new(None).unwrap();

    let key = b"exists";
    assert!(!db.contains_key(key));

    db.insert(key, b"value").unwrap();
    assert!(db.contains_key(key));

    db.delete(key).unwrap();
    assert!(!db.contains_key(key));
}

#[test]
fn test_get_size() {
    let db = FunKV::new(None).unwrap();

    let key = b"sized_key";
    let value = vec![b'x'; 12345];

    db.insert(key, &value).unwrap();

    let size = db.get_size(key).unwrap();
    assert_eq!(size, 12345);

    assert!(db.get_size(b"non_exists_key").is_err());
}