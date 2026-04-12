/// Per-physical-disk I/O snapshot.
#[derive(Clone, Debug, Default)]
pub struct DiskSnapshot {
    /// e.g. "PhysicalDrive0" or "0 C: D:"
    pub device_name: String,
    pub read_bps: u64,
    pub write_bps: u64,
    /// From PDH "% Disk Time" counter (0–100).
    pub utilization_pct: f32,
}
