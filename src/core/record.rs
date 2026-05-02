use std::{
    mem,
    sync::atomic::{self, AtomicU32, AtomicU64, Ordering},
};

use bytes::Bytes;
use crossbeam_epoch::{Atomic, Guard, Shared};
use parking_lot::RwLock;

#[repr(C)]
#[derive(Debug)]
pub struct Record {
    pub key: Vec<u8>,
    pub value: RwLock<Option<Bytes>>,

    pub ttl: AtomicU64,
    pub timestamp: u64,
    pub value_len: usize,
    pub sector: AtomicU64,
    pub refcount: AtomicU32,
    pub key_len: u16,

    pub hash_link: AtomicLink,
    pub cache_ref_bit: AtomicU32,
    pub cache_access_time: AtomicU64,
}

pub struct AtomicLink {
    pub next: Atomic<Record>,
}

impl std::fmt::Debug for AtomicLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtomicLink")
            .field("next", &"<atomic>")
            .finish()
    }
}

impl Default for AtomicLink {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicLink {
    pub fn new() -> Self {
        Self {
            next: Atomic::null(),
        }
    }

    pub fn load<'g>(&self, guard: &'g Guard) -> Option<Shared<'g, Record>> {
        let ptr = self.next.load(Ordering::Acquire, guard);

        if ptr.is_null() { None } else { Some(ptr) }
    }

    pub fn store(&self, record: Option<Shared<Record>>, _guard: &Guard) {
        let ptr = record.unwrap_or(Shared::null());
        self.next.store(ptr, Ordering::Release);
    }
}

unsafe impl Send for Record {}
unsafe impl Sync for Record {}

impl Record {
    pub fn new(key: Vec<u8>, value: Bytes, timestamp: u64) -> Self {
        let key_len = key.len() as u16;
        let value_len = value.len();

        Self {
            key,
            value: parking_lot::RwLock::new(Some(value)),
            ttl: AtomicU64::new(0),

            timestamp,
            value_len,
            sector: AtomicU64::new(0),
            refcount: AtomicU32::new(1),
            key_len,

            hash_link: AtomicLink::new(),
            cache_ref_bit: AtomicU32::new(0),
            cache_access_time: AtomicU64::new(0),
        }
    }

    pub fn new_with_ttl(key: Vec<u8>, value: Bytes, timestamp: u64, ttl: u64) -> Self {
        let record = Self::new(key, value, timestamp);
        record.ttl.store(ttl, Ordering::Release);

        record
    }

    pub fn calculate_size(&self) -> usize {
        mem::size_of::<Self>() + self.key.capacity() + self.value_len
    }

    #[inline]
    pub fn get_value(&self) -> Option<Bytes> {
        self.value.read().clone()
    }

    #[inline]
    pub fn clear_value(&self) {
        *self.value.write() = None;
        atomic::fence(Ordering::Release);
    }

    pub fn inc_ref(&self) {
        self.refcount.fetch_add(1, Ordering::AcqRel);
    }

    pub fn dec_ref(&self) -> u32 {
        let old = self.refcount.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(old > 0, "Record refcount underflow");

        old - 1
    }

    pub fn ref_count(&self) -> u32 {
        self.refcount.load(Ordering::Acquire)
    }
}
