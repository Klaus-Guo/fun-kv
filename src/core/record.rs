use std::{mem, sync::atomic::{self, AtomicU32, AtomicU64, Ordering}};

use crossbeam_epoch::{Atomic, Guard, Shared};
use parking_lot::RwLock;

use crate::{constants::*, core::FunKV};

#[repr(C)]
#[derive(Debug)]
pub struct Record {
    pub key: Vec<u8>,
    pub value: RwLock<Option<Vec<u8>>>,

    pub ttl_expiry: AtomicU64,
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

    pub fn compare_exchange<'g>(
        &self,
        current: Shared<'g, Record>,
        new: Shared<'g, Record>,
        guard: &'g Guard,
    ) -> Result<Shared<'g, Record>, Shared<'g, Record>> {
        self.next
            .compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire, guard)
            .map_err(|e| e.current)
    }
}

unsafe impl Send for Record {}
unsafe impl Sync for Record {}

impl Record {
    pub fn new(key: Vec<u8>, value: Vec<u8>, timestamp: u64) -> Self {
        let key_len = key.len() as u16;
        let value_len = value.len();

        Self {
            key,
            value: parking_lot::RwLock::new(Some(value)),
            ttl_expiry: AtomicU64::new(0),

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

    pub fn new_with_ttl(key: Vec<u8>, value: Vec<u8>, timestamp: u64, ttl_expiry: u64) -> Self {
        let record = Self::new(key, value, timestamp);
        record.ttl_expiry.store(ttl_expiry, Ordering::Release);

        record
    }

    pub fn calculate_size(&self) -> usize {
        FunKV::calculate_record_size(self.key.capacity(), self.value_len)
    }

    pub fn calculate_disk_size(&self) -> usize {
        let record_size = SECTOR_HEADER_SIZE +
                         mem::size_of::<u16>() + // key_len
                         self.key_len as usize +
                         mem::size_of::<usize>() + // value_len
                         mem::size_of::<u64>() + // timestamp
                         self.value_len;

        // Round up to block size
        record_size.div_ceil(BLOCK_SIZE) * BLOCK_SIZE
    }

    #[inline]
    pub fn get_value(&self) -> Option<Vec<u8>> {
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
