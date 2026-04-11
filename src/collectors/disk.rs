use crate::models::disk::DiskSnapshot;

/// Disk I/O collector using Windows PDH (Performance Data Helper).
/// Full PDH implementation is Phase 2. This stub returns empty data
/// so Phase 1 compiles and runs without requiring PDH setup.
pub struct DiskCollector;

impl DiskCollector {
    pub fn new() -> Self {
        Self
    }

    pub fn collect(&mut self) -> Vec<DiskSnapshot> {
        // TODO Phase 2: initialize PDH query with localized counter paths
        // (PdhLookupPerfIndexByNameW), then PdhCollectQueryData each tick.
        Vec::new()
    }
}
