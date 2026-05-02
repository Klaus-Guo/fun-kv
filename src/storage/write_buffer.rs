use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicUsize},
    },
    time::Instant,
};

use parking_lot::Mutex;

use crate::{constants::Operation, core::record::Record};

#[repr(align(64))]
pub struct ShardedWriteBuffer {
    buffer: Mutex<VecDeque<WriteEntry>>,

    count: AtomicUsize,

    size: AtomicUsize,
}

pub struct WriteEntry {
    pub oper: Operation,
    pub record: Arc<Record>,
    pub old_value_len: usize,
    pub work_status: AtomicU32,
    pub retry_count: AtomicU32,
    pub timestamp: Instant,
}
