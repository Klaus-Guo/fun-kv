use crate::core::builder::DbBuilder;

#[test]
fn test_builder_configuration() {
    let db = DbBuilder::new()
        .max_memory(1_000_000_000)
        .hash_bits(20)
        .enable_caching(false)
        .build()
        .unwrap();

    db.insert(b"testkey", b"testvalue").unwrap();
    assert_eq!(db.get(b"testkey").unwrap(), b"testvalue");
}

#[test]
fn test_builder_default() {
    let db = DbBuilder::new().build().unwrap();

    db.insert(b"testkey", b"testvalue").unwrap();
    assert_eq!(db.get(b"testkey").unwrap(), b"testvalue");
}