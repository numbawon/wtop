use crate::models::RingBuffer;

/// RAM, swap, and Windows commit charge snapshot.
#[derive(Clone, Debug)]
pub struct MemSnapshot {
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    /// Windows commit charge: virtual memory committed system-wide.
    pub commit_total_bytes: u64,
    pub commit_limit_bytes: u64,
    /// Rolling RAM usage % history for sparkline rendering.
    pub ram_history: RingBuffer<f32>,
    /// Rolling commit charge % history for sparkline rendering.
    pub commit_history: RingBuffer<f32>,
}

impl MemSnapshot {
    pub fn new(history_len: usize) -> Self {
        Self {
            ram_used_bytes: 0,
            ram_total_bytes: 0,
            swap_used_bytes: 0,
            swap_total_bytes: 0,
            commit_total_bytes: 0,
            commit_limit_bytes: 0,
            ram_history: RingBuffer::new(history_len),
            commit_history: RingBuffer::new(history_len),
        }
    }

    pub fn ram_pct(&self) -> f64 {
        if self.ram_total_bytes == 0 {
            return 0.0;
        }
        self.ram_used_bytes as f64 / self.ram_total_bytes as f64 * 100.0
    }

    pub fn swap_pct(&self) -> f64 {
        if self.swap_total_bytes == 0 {
            return 0.0;
        }
        self.swap_used_bytes as f64 / self.swap_total_bytes as f64 * 100.0
    }

    pub fn commit_pct(&self) -> f64 {
        if self.commit_limit_bytes == 0 {
            return 0.0;
        }
        self.commit_total_bytes as f64 / self.commit_limit_bytes as f64 * 100.0
    }
}
