use std::io;

use thiserror::Error;

use crate::constants::*;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Key not found")]
    KeyNotFound,

    #[error("Out of memory")]
    OutOfMemory,

    #[error("Allocation failed")]
    AllocationFailed,

    #[error("Timestamp is older than existing record")]
    OlderTimestamp,

    #[error("Invalid argument")]
    InvalidArgument,

    #[error("Invalid device")]
    InvalidDevice,

    #[error("Invalid key size: The size of key must between 1~{}", MAX_KEY_SIZE)]
    InvalidKeySize,

    #[error(
        "Invalid value size: The size of key must between 1~{}",
        MAX_VALUE_SIZE
    )]
    InvalidValueSize,

    #[error("Database is full")]
    DatabaseFull,

    #[error("Duplicate key")]
    DuplicateKey,

    #[error("Corrupted data")]
    CorruptedData,

    #[error("Out of space")]
    OutOfSpace,

    #[error("Numeric overflow")]
    NumericOverflow,

    #[error("Size mismatch: expected {expected} bytes, got {actual} bytes")]
    SizeMismatch { expected: usize, actual: usize },

    #[error("System error: {0}")]
    SystemError(i32),

    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("System shutting down")]
    ShuttingDown,

    #[error("Channel error")]
    ChannelError,
}

pub type Result<T> = std::result::Result<T, DbError>;

impl From<i32> for DbError {
    fn from(errno: i32) -> Self {
        match errno {
            2 => DbError::KeyNotFound,
            12 => DbError::OutOfMemory,
            17 => DbError::OlderTimestamp,
            22 => DbError::InvalidKeySize,
            28 => DbError::DatabaseFull,
            75 => DbError::NumericOverflow,
            90 => DbError::SizeMismatch {
                expected: 0,
                actual: 0,
            },
            _ => DbError::SystemError(errno),
        }
    }
}

impl DbError {
    pub fn errno(&self) -> i32 {
        match self {
            DbError::InvalidKeySize | DbError::InvalidValueSize => EINVAL,
            DbError::KeyNotFound => ENOENT,
            DbError::DatabaseFull => ENOSPC,
            DbError::OutOfMemory | DbError::AllocationFailed => ENOMEM,
            DbError::OlderTimestamp => EEXIST,
            DbError::NumericOverflow => EOVERFLOW,
            DbError::SystemError(e) => *e,
            DbError::SizeMismatch { .. } => EMSGSIZE,
            DbError::IoError(_) => EIO,
            _ => EIO,
        }
    }
}
