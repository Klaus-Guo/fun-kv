use std::time::Duration;

use crate::core::ttl::TtlConfig;

pub struct DbConfig {
    pub persistency: bool,
    pub max_memory: Option<usize>,
    pub enable_caching: bool,
    pub hash_bit: u32,
    pub enable_ttl: bool,
    pub ttl_config: Option<TtlConfig>,
    pub file_path: Option<String>,
    pub file_size: Option<u64>,
}

pub struct DbBuilder {
    max_memory: Option<usize>,
    enable_caching: Option<bool>,
    hash_bit: u32,
    enable_ttl: bool,
    ttl_config: Option<TtlConfig>,
    file_path: Option<String>,
    file_size: Option<u64>,
}

impl DbBuilder {
    pub fn new() -> Self {
        Self {
            max_memory: Some(4 * 1024 * 1024 * 1024),   // TODO: Const table
            enable_caching: None,
            hash_bit: 23,                               // TODO: Const table
            enable_ttl: false,
            ttl_config: None,
            file_path: None,
            file_size: None,
        }
    }

    pub fn max_memory(mut self, limit: usize) -> Self {
        self.max_memory = Some(limit);
        self
    }

    pub fn no_memory_limit(mut self) -> Self {
        self.max_memory = None;
        self
    }

    pub fn enable_caching(mut self, enable: bool) -> Self {
        self.enable_caching = Some(enable);
        self
    }

    pub fn hash_bit(mut self, bits: u32) -> Self {
        self.hash_bit = bits;
        self
    }

    pub fn enable_ttl(mut self, enable: bool) -> Self {
        self.enable_ttl = enable;
        if enable {
            let mut config = self.ttl_config.unwrap_or_default();
            config.enabled = true;
            self.ttl_config = Some(config);
        }
        self
    }

    pub fn ttl_config(
        mut self, 
        interval: u64,
        max_time: u64,
        threshold: f32,
        sample_size: usize
    ) -> Self {
        self.ttl_config = Some(TtlConfig { 
            enabled: true,
            interval: Duration::from_millis(interval), 
            max_time_per_run: Duration::from_millis(max_time), 
            max_iterations: 16, 
            expiry_threshold: threshold, 
            sample_size 
        });
        self
    }

    pub fn file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    pub fn file_size(mut self, size: u64) -> Self {
        self.file_size = Some(size);
        self
    }

    // TODO: parameterization a custom ttl_config

    pub fn build(self) -> Option<()> {
        let persistency = self.file_path.is_none();

        let config = DbConfig {
            persistency: persistency,
            max_memory: self.max_memory,
            enable_caching: self.enable_caching.unwrap_or(!persistency),
            hash_bit: self.hash_bit,
            enable_ttl: self.enable_ttl, 
            ttl_config: self.ttl_config,
            file_path: self.file_path, 
            file_size: self.file_size,
        };

        // TODO: FunKV::build_with_config(config)
        None
    }
}

impl Default for DbBuilder {
    fn default() -> Self {
        Self::new()
    }
}