/// RAM, swap, and Windows commit charge snapshot.
#[derive(Clone, Debug, Default)]
pub struct MemSnapshot {
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    /// Windows commit charge: virtual memory committed system-wide.
    pub commit_total_bytes: u64,
    pub commit_limit_bytes: u64,
}

impl MemSnapshot {
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
