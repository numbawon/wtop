use std::collections::VecDeque;

/// Fixed-capacity ring buffer used for sparkline history.
#[derive(Clone, Debug)]
pub struct RingBuffer<T> {
    pub data: VecDeque<T>,
    pub capacity: usize,
}

impl<T: Copy + Default> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, val: T) {
        if self.data.len() == self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(val);
    }

    /// Returns a Vec suitable for passing to ratatui Sparkline (which wants u64).
    pub fn as_u64_vec(&self) -> Vec<u64>
    where
        T: Into<f64> + Copy,
    {
        self.data.iter().map(|v| (*v).into() as u64).collect()
    }
}

/// Snapshot of a single logical CPU core.
#[derive(Clone, Debug)]
pub struct CoreSnapshot {
    pub index: usize,
    pub usage_pct: f32,
    pub frequency_mhz: u64,
    /// Recent usage history for sparkline rendering.
    pub history: RingBuffer<f32>,
}

/// Full CPU snapshot including all cores.
#[derive(Clone, Debug)]
pub struct CpuSnapshot {
    pub cores: Vec<CoreSnapshot>,
    pub aggregate_pct: f32,
    pub aggregate_history: RingBuffer<f32>,
    pub logical_count: usize,
    pub physical_count: usize,
    pub brand: String,
}

impl CpuSnapshot {
    pub fn new(history_len: usize) -> Self {
        Self {
            cores: Vec::new(),
            aggregate_pct: 0.0,
            aggregate_history: RingBuffer::new(history_len),
            logical_count: 0,
            physical_count: 0,
            brand: String::new(),
        }
    }
}
