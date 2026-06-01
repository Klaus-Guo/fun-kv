use std::{sync::Arc, thread};

use crate::core::FunKV;

#[test]
fn test_concurrent_inserts() {
    let db = Arc::new(FunKV::new(None).unwrap());
    let mut handles = vec![];

    for i in 0..10 {
        let db_clone = Arc::clone(&db);
        handles.push(thread::spawn(move || {
            for j in 0..100 {
                let key = format!("thread{}:key{}", i, j);
                let value = format!("thread{}:value{}", i, j);

                db_clone.insert(key.as_bytes(), value.as_bytes()).unwrap();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(db.len(), 1000);
}

#[test]
fn test_concurrent_mixed_operations() {
    let db = Arc::new(FunKV::new(None).unwrap());

    for i in 0..100 {
        let key = format!("key_{}", i);
        let value = format!("value_{}", i);
        db.insert(key.as_bytes(), value.as_bytes()).unwrap();
    }

    let mut handles = vec![];

    for _ in 0..5 {
        let db_clone = Arc::clone(&db);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let key = format!("key_{}", i);
                let _ = db_clone.get(key.as_bytes());
            }
        }));
    }

    for t in 0..5 {
        let db_clone = Arc::clone(&db);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let key = format!("key_{}", i);
                let value = format!("updated_by_{}", t);
                let _ = db_clone.insert(key.as_bytes(), value.as_bytes());
            }
        }));
    }

    for _ in 0..2 {
        let db_clone = Arc::clone(&db);
        handles.push(thread::spawn(move || {
            for i in 90..100 {
                let key = format!("key_{}", i);
                let _ = db_clone.delete(key.as_bytes());
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}