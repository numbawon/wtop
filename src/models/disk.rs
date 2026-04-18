use crate::models::RingBuffer;

/// Per-logical-drive I/O snapshot (one row per drive letter).
#[derive(Clone, Debug)]
pub struct DiskSnapshot {
    /// Drive letter label, e.g. "C:" or "D:".
    pub drive: String,
    /// Read bytes/sec from PDH \LogicalDisk.
    pub read_bps: u64,
    /// Write bytes/sec from PDH \LogicalDisk.
    pub write_bps: u64,
    /// From PDH "% Disk Time" counter (0–100).
    pub utilization_pct: f32,
    /// Free bytes on this drive.
    pub free_bytes: u64,
    /// Total capacity bytes on this drive.
    pub total_bytes: u64,
    /// Rolling utilization % history for sparkline rendering.
    pub util_history: RingBuffer<f32>,
}

