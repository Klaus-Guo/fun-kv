use bytemuck::{Pod, Zeroable};

use std::{
    mem::{self, MaybeUninit},
    slice::from_raw_parts,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::constants::*;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub signature: [u8; SIGNATURE_SIZE],
    pub version: u32,
    pub total_records: u64,
    pub total_size: u64,
    pub persistent_size: u64,
    pub block_size: u32,
    pub fragmentation: u32,
    pub creation_time: u64,
    pub last_update_time: u64,
    reserved: [u8; RESERVED_SIZE],
}

impl Default for Metadata {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Zeroable for Metadata {}
unsafe impl Pod for Metadata {}

impl Metadata {
    pub fn new() -> Self {
        let now = Self::get_time_now();

        let mut this = MaybeUninit::<Self>::zeroed();
        let ptr = this.as_mut_ptr();

        unsafe {
            (*ptr).signature = *SIGNATURE;
            (*ptr).version = METADATA_VERSION;
            (*ptr).block_size = BLOCK_SIZE as u32;
            (*ptr).creation_time = now;
            (*ptr).last_update_time = now;
        }

        unsafe { this.assume_init() }
    }

    pub fn validate(&self) -> bool {
        if self.signature != *SIGNATURE {
            return false;
        }

        if self.block_size != BLOCK_SIZE as u32 {
            return false;
        }

        if self.persistent_size == 0 || self.persistent_size > MAX_PERSISTENT_SIZE as u64 {
            return false;
        }

        // Skip checksum validation to avoid alignment issues
        true
    }

    pub fn update(&mut self) {
        self.last_update_time = Self::get_time_now();
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }

    pub fn from_byte(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < mem::size_of::<Self>() {
            return None;
        }

        let metadata = bytemuck::try_from_bytes::<Metadata>(bytes).ok().copied();

        if metadata.map_or(false, |metadata| metadata.validate()) {
            metadata
        } else {
            None
        }
    }

    #[deprecated(
        note = "This method is retained for potential future rollbacks; its use is strongly discouraged."
    )]
    fn as_bytes_unsafe(&self) -> &[u8] {
        unsafe { from_raw_parts(self as *const Self as *const u8, mem::size_of::<Self>()) }
    }

    fn get_time_now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs()
    }
}
