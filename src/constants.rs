pub const KB: usize = 1024;
pub const MB: usize = 1024 * KB;
pub const GB: usize = 1024 * MB;
pub const TB: usize = 1024 * GB;

pub const MAX_KEY_SIZE: usize = 100 * KB;
pub const MAX_VALUE_SIZE: usize = 4 * MB;
pub const DEFAULT_MAX_MEMORY: usize = 4 * GB;

pub const DEFAULT_PERSISTENT_SIZE: usize = 4 * GB;
pub const MAX_PERSISTENT_SIZE: usize = TB;

pub const DEFAULT_HASH_BITS: u32 = 23;
pub const DEFAULT_ITERATION: usize = 16;
pub const CACHE_BUCKETS: usize = 16384;

// update this when changing the metadata structure
pub const METADATA_VERSION: u32 = 1;
pub const SIGNATURE: &[u8; SIGNATURE_SIZE] = b"FUNKV_SIG";
pub const SIGNATURE_SIZE: usize = 9;
pub const RESERVED_SIZE: usize = 68;
pub const METADATA_BLOCK_SIZE: u64 = 16;

pub const BLOCK_SIZE: usize = 4096;
pub const SECTOR_HEADER_SIZE: usize = 4;

pub const CACHE_HIGH_WATERMARK_MB: usize = 100;
pub const CACHE_LOW_WATERMARK_MB: usize = 50;
pub const MAX_SCANS: usize = 3;
pub const CACHE_MAX_SIZE: usize = GB;

// POSIX error codes
pub const EINVAL: i32 = 22;
pub const ENOENT: i32 = 2;
pub const ENOSPC: i32 = 28;
pub const ENOMEM: i32 = 12;
pub const EEXIST: i32 = 17;
pub const EOVERFLOW: i32 = 75;
pub const EMSGSIZE: i32 = 90;
pub const EIO: i32 = 5;

// Operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Insert,
    Update,
    Delete,
    Get,
    PartialUpdate,
}

pub const MALLOC_LIMIT: usize = 8192;
pub const LARGE_ALLOC_THRESHOLD: usize = 8192;

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_MASK: usize = PAGE_SIZE - 1;