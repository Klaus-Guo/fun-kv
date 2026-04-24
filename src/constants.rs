pub const KB: usize = 1024;
pub const MB: usize = 1024 * KB;
pub const GB: usize = 1024 * MB;

pub const MAX_KEY_SIZE: usize = 100 * KB;
pub const MAX_VALUE_SIZE: usize = 4 * MB;
pub const DEFAULT_MAX_MEMORY: usize = 4 * GB;

pub const DEFAULT_HASH_BITS: u32 = 23;
pub const DEFAULT_ITERATION: usize = 16;

// POSIX error codes
pub const EINVAL: i32 = 22;
pub const ENOENT: i32 = 2;
pub const ENOSPC: i32 = 28;
pub const ENOMEM: i32 = 12;
pub const EEXIST: i32 = 17;
pub const EOVERFLOW: i32 = 75;
pub const EMSGSIZE: i32 = 90;
pub const EIO: i32 = 5;