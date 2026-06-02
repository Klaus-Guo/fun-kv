use crate::{core::FunKV, error::DbError};

#[test]
fn test_atomic_increment_basic() {
    let db = FunKV::new(None).unwrap();

    let key = b"counter";

    db.insert(key, &0i64.to_le_bytes()).unwrap();

    let val = db.atomic_increment(key, 1).unwrap();
    assert_eq!(val, 1);
    let val = db.atomic_increment(key, 5).unwrap();
    assert_eq!(val, 6);
    let val = db.atomic_increment(key, -2).unwrap();
    assert_eq!(val, 4);
}

#[test]
fn test_atomic_increment_create_if_not_exists() {
    let db = FunKV::new(None).unwrap();

    let val = db.atomic_increment(b"new_counter", 100).unwrap();
    assert_eq!(val, 100);
    let val = db.atomic_increment(b"new_counter", 50).unwrap();
    assert_eq!(val, 150);
}

#[test]
fn test_atomic_increment_invalid_value() {
    let db = FunKV::new(None).unwrap();

    db.insert(b"wrong_counter", b"text").unwrap();

    let result = db.atomic_increment(b"wrong_counter", 1);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DbError::InvalidOperation));
}

#[test]
fn test_atomic_increment_saturation() {
    let db = FunKV::new(None).unwrap();

    let key = b"saturate";

    let value = i64::MAX - 1;
    db.insert(key, &value.to_le_bytes()).unwrap();

    let result = db.atomic_increment(key, 10).unwrap();
    assert_eq!(result, i64::MAX);
}