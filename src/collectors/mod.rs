pub mod cpu;
pub mod disk;
pub mod memory;
pub mod network;
pub mod process;
pub mod thread;

use crate::config::Config;
use crate::models::{
    cpu::CpuSnapshot,
    disk::DiskSnapshot,
    memory::MemSnapshot,
    network::NetSnapshot,
    process::ProcessEntry,
    thread::ThreadEntry,
};
use std::sync::{mpsc, Arc, RwLock};
use std::thread::{spawn as spawn_thread, sleep as sleep_thread};
use std::time::Duration;

/// Shared state handles the UI thread reads from.
pub struct CollectorHub {
    pub cpu: Arc<RwLock<CpuSnapshot>>,
    pub memory: Arc<RwLock<MemSnapshot>>,
    pub processes: Arc<RwLock<Vec<ProcessEntry>>>,
    pub disks: Arc<RwLock<Vec<DiskSnapshot>>>,
    pub networks: Arc<RwLock<Vec<NetSnapshot>>>,
    /// Send a PID here to request thread expansion.
    pub thread_request_tx: mpsc::Sender<u32>,
    /// Receive (pid, threads) results here.
    pub thread_result_rx: mpsc::Receiver<(u32, Vec<ThreadEntry>)>,
}

impl CollectorHub {
    /// Spawn all background collector threads and return the hub.
    pub fn spawn(config: &Config) -> Self {
        let cpu_state: Arc<RwLock<CpuSnapshot>> =
            Arc::new(RwLock::new(CpuSnapshot::new(config.cpu_history_len)));
        let mem_state: Arc<RwLock<MemSnapshot>> = Arc::new(RwLock::default());
        let proc_state: Arc<RwLock<Vec<ProcessEntry>>> = Arc::new(RwLock::new(Vec::new()));
        let disk_state: Arc<RwLock<Vec<DiskSnapshot>>> = Arc::new(RwLock::new(Vec::new()));
        let net_state: Arc<RwLock<Vec<NetSnapshot>>> = Arc::new(RwLock::new(Vec::new()));

        let interval_ms = config.refresh_interval_ms;
        let history_len = config.cpu_history_len;

        // CPU collector thread.
        {
            let state = Arc::clone(&cpu_state);
            spawn_thread(move || {
                let mut collector = cpu::CpuCollector::new(history_len);
                loop {
                    let snapshot = collector.collect();
                    if let Ok(mut s) = state.write() {
                        // Preserve per-core histories across snapshots.
                        for (i, core) in snapshot.cores.iter().enumerate() {
                            if let Some(existing) = s.cores.get_mut(i) {
                                existing.usage_pct = core.usage_pct;
                                existing.frequency_mhz = core.frequency_mhz;
                                existing.history.push(core.usage_pct);
                            } else {
                                s.cores.push(core.clone());
                                if let Some(c) = s.cores.last_mut() {
                                    c.history.push(c.usage_pct);
                                }
                            }
                        }
                        s.aggregate_pct = snapshot.aggregate_pct;
                        s.aggregate_history.push(snapshot.aggregate_pct);
                        s.logical_count = snapshot.logical_count;
                        s.physical_count = snapshot.physical_count;
                        s.brand = snapshot.brand;
                    }
                    sleep_thread(Duration::from_millis(interval_ms));
                }
            });
        }

        // Memory collector thread.
        {
            let state = Arc::clone(&mem_state);
            spawn_thread(move || {
                let mut collector = memory::MemCollector::new();
                loop {
                    let snapshot = collector.collect();
                    if let Ok(mut s) = state.write() {
                        *s = snapshot;
                    }
                    sleep_thread(Duration::from_millis(interval_ms));
                }
            });
        }

        // Channels for thread expansion requests.
        let (thread_req_tx, thread_req_rx) = mpsc::channel::<u32>();
        let (thread_res_tx, thread_res_rx) = mpsc::channel::<(u32, Vec<ThreadEntry>)>();

        // Process collector thread.
        {
            let state = Arc::clone(&proc_state);
            spawn_thread(move || {
                let mut collector =
                    process::ProcessCollector::new(thread_req_rx, thread_res_tx);
                loop {
                    let mut snapshot = collector.collect();
                    if let Ok(mut s) = state.write() {
                        // Preserve expanded state and cached thread lists across
                        // refreshes so open rows don't collapse every tick.
                        for entry in snapshot.iter_mut() {
                            if let Some(old) = s.iter().find(|p| p.pid == entry.pid) {
                                entry.expanded = old.expanded;
                                if !old.threads.is_empty() {
                                    entry.threads = old.threads.clone();
                                }
                            }
                        }
                        *s = snapshot;
                    }
                    sleep_thread(Duration::from_millis(interval_ms * 2));
                }
            });
        }

        // Disk collector thread.
        {
            let state = Arc::clone(&disk_state);
            spawn_thread(move || {
                let mut collector = disk::DiskCollector::new();
                loop {
                    let snapshot = collector.collect();
                    if let Ok(mut s) = state.write() {
                        *s = snapshot;
                    }
                    sleep_thread(Duration::from_millis(interval_ms));
                }
            });
        }

        // Network collector thread.
        {
            let state = Arc::clone(&net_state);
            spawn_thread(move || {
                let mut collector = network::NetCollector::new();
                loop {
                    let snapshot = collector.collect();
                    if let Ok(mut s) = state.write() {
                        *s = snapshot;
                    }
                    sleep_thread(Duration::from_millis(interval_ms));
                }
            });
        }

        Self {
            cpu: cpu_state,
            memory: mem_state,
            processes: proc_state,
            disks: disk_state,
            networks: net_state,
            thread_request_tx: thread_req_tx,
            thread_result_rx: thread_res_rx,
        }
    }
}
