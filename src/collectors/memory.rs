use crate::models::memory::MemSnapshot;
use sysinfo::System;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

pub struct MemCollector {
    sys: System,
}

impl MemCollector {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        Self { sys }
    }

    pub fn collect(&mut self) -> MemSnapshot {
        self.sys.refresh_memory();

        let ram_total = self.sys.total_memory();
        let ram_used = self.sys.used_memory();
        let swap_total = self.sys.total_swap();
        let swap_used = self.sys.used_swap();

        // Retrieve Windows commit charge via GlobalMemoryStatusEx.
        let (commit_total, commit_limit) = self.get_commit_charge();

        MemSnapshot {
            ram_used_bytes: ram_used,
            ram_total_bytes: ram_total,
            swap_used_bytes: swap_used,
            swap_total_bytes: swap_total,
            commit_total_bytes: commit_total,
            commit_limit_bytes: commit_limit,
        }
    }

    fn get_commit_charge(&self) -> (u64, u64) {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // Safety: we pass a properly initialized struct.
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        if ok.is_ok() {
            let commit_total = status.ullTotalPageFile - status.ullAvailPageFile;
            let commit_limit = status.ullTotalPageFile;
            (commit_total, commit_limit)
        } else {
            (0, 0)
        }
    }
}
