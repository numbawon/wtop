use crate::models::cpu::{CoreSnapshot, CpuSnapshot, RingBuffer};
use sysinfo::System;
use std::time::{Duration, Instant};

pub struct CpuCollector {
    sys: System,
    history_len: usize,
    last_collect: Option<Instant>,
}

impl CpuCollector {
    pub fn new(history_len: usize) -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_all();
        Self {
            sys,
            history_len,
            last_collect: None,
        }
    }

    /// Collect a fresh CPU snapshot.
    /// On the very first call we warm up sysinfo (it needs two calls ~200ms apart).
    pub fn collect(&mut self) -> CpuSnapshot {
        // First call — seed the counters and return zeroed snapshot.
        if self.last_collect.is_none() {
            self.sys.refresh_cpu_all();
            self.last_collect = Some(Instant::now());
            return CpuSnapshot::new(self.history_len);
        }

        // Ensure at least 200ms between refreshes so deltas are meaningful.
        let elapsed = self.last_collect.unwrap().elapsed();
        if elapsed < Duration::from_millis(200) {
            std::thread::sleep(Duration::from_millis(200) - elapsed);
        }

        self.sys.refresh_cpu_all();
        self.last_collect = Some(Instant::now());

        let cpus = self.sys.cpus();
        let logical_count = cpus.len();

        let mut snapshot = CpuSnapshot::new(self.history_len);
        snapshot.logical_count = logical_count;
        snapshot.physical_count = self.sys.physical_core_count().unwrap_or(logical_count);
        snapshot.brand = cpus.first().map(|c| c.brand().to_string()).unwrap_or_default();

        let mut total_usage = 0.0f32;
        for (i, cpu) in cpus.iter().enumerate() {
            let usage = cpu.cpu_usage();
            total_usage += usage;
            snapshot.cores.push(CoreSnapshot {
                index: i,
                usage_pct: usage,
                frequency_mhz: cpu.frequency(),
                history: RingBuffer::new(self.history_len),
            });
        }

        snapshot.aggregate_pct = if logical_count > 0 {
            total_usage / logical_count as f32
        } else {
            0.0
        };
        snapshot.aggregate_history.push(snapshot.aggregate_pct);

        snapshot
    }
}
