use std::time::Duration;

#[derive(Clone, Debug)]
pub struct TtlConfig {
    pub enabled: bool,
    pub interval: Duration,
    pub max_time_per_run: Duration,
    pub max_iterations: usize,
    pub expiry_threshold: f32,
    pub sample_size: usize,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: Duration::from_millis(1000),
            max_time_per_run: Duration::from_millis(1),
            max_iterations: 16,
            expiry_threshold: 0.25,
            sample_size: 100,
        }
    }
}

impl TtlConfig {
    pub fn default() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }
}
