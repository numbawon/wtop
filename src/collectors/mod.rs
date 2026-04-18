pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod inspect;
pub mod memory;
pub mod network;
pub mod process;
pub mod services;
pub mod thread;

use std::collections::HashMap;
use crate::config::Config;
use crate::models::{
    cpu::CpuSnapshot,
    disk::DiskSnapshot,
    gpu::GpuAdapter,
    memory::MemSnapshot,
    network::NetSnapshot,
    process::ProcessEntry,
    services::ServiceEntry,
    thread::ThreadEntry,
};
use std::sync::{mpsc, Arc, RwLock};
use std::thread::{sleep as sleep_thread, Builder as ThreadBuilder, JoinHandle};
use std::time::Duration;

/// Shared state handles the UI thread reads from.
pub struct CollectorHub {
    pub cpu: Arc<RwLock<CpuSnapshot>>,
    pub memory: Arc<RwLock<MemSnapshot>>,
    pub processes: Arc<RwLock<Vec<ProcessEntry>>>,
    pub disks: Arc<RwLock<Vec<DiskSnapshot>>>,
    pub networks: Arc<RwLock<Vec<NetSnapshot>>>,
    pub gpus: Arc<RwLock<Vec<GpuAdapter>>>,
    pub services: Arc<RwLock<Vec<ServiceEntry>>>,
    /// Send a PID here to request thread expansion.
    pub thread_request_tx: mpsc::Sender<u32>,
    /// Receive (pid, threads) results here.
    pub thread_result_rx: mpsc::Receiver<(u32, Vec<ThreadEntry>)>,
    /// Handles for the background collector threads.
    /// Used only to detect unexpected termination (panic).
    collector_handles: Vec<(&'static str, JoinHandle<()>)>,
}

impl CollectorHub {
    /// Returns the names of any collector threads that have stopped.
    /// A stopped thread means it panicked, since every loop is infinite.
    pub fn dead_collectors(&self) -> Vec<&'static str> {
        self.collector_handles
            .iter()
            .filter(|(_, h)| h.is_finished())
            .map(|(name, _)| *name)
            .collect()
    }

    /// Spawn all background collector threads and return the hub.
    pub fn spawn(config: &Config) -> Self {
        let cpu_state: Arc<RwLock<CpuSnapshot>> =
            Arc::new(RwLock::new(CpuSnapshot::new(config.cpu_history_len)));
        let mem_state: Arc<RwLock<MemSnapshot>> =
            Arc::new(RwLock::new(MemSnapshot::new(config.cpu_history_len)));
        let proc_state: Arc<RwLock<Vec<ProcessEntry>>> = Arc::new(RwLock::new(Vec::new()));
        let disk_state: Arc<RwLock<Vec<DiskSnapshot>>> = Arc::new(RwLock::new(Vec::new()));
        let net_state: Arc<RwLock<Vec<NetSnapshot>>> = Arc::new(RwLock::new(Vec::new()));
        let svc_state: Arc<RwLock<Vec<ServiceEntry>>> = Arc::new(RwLock::new(Vec::new()));
        let gpu_state: Arc<RwLock<Vec<GpuAdapter>>> = Arc::new(RwLock::new(Vec::new()));

        let interval_ms = config.refresh_interval_ms;
        let history_len = config.cpu_history_len;
        let mut handles: Vec<(&'static str, JoinHandle<()>)> = Vec::new();

        {
            let state = Arc::clone(&cpu_state);
            let h = ThreadBuilder::new()
                .name("wtop-cpu".into())
                .spawn(move || {
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
                })
                .expect("failed to spawn cpu collector thread");
            handles.push(("cpu", h));
        }

        {
            let state = Arc::clone(&mem_state);
            let h = ThreadBuilder::new()
                .name("wtop-memory".into())
                .spawn(move || {
                    let mut collector = memory::MemCollector::new();
                    loop {
                        let snap = collector.collect();
                        if let Ok(mut s) = state.write() {
                            let ram_pct = if snap.ram_total_bytes > 0 {
                                (snap.ram_used_bytes as f32 / snap.ram_total_bytes as f32) * 100.0
                            } else {
                                0.0
                            };
                            let commit_pct = if snap.commit_limit_bytes > 0 {
                                (snap.commit_total_bytes as f32 / snap.commit_limit_bytes as f32) * 100.0
                            } else {
                                0.0
                            };
                            s.ram_used_bytes = snap.ram_used_bytes;
                            s.ram_total_bytes = snap.ram_total_bytes;
                            s.swap_used_bytes = snap.swap_used_bytes;
                            s.swap_total_bytes = snap.swap_total_bytes;
                            s.commit_total_bytes = snap.commit_total_bytes;
                            s.commit_limit_bytes = snap.commit_limit_bytes;
                            s.ram_history.push(ram_pct);
                            s.commit_history.push(commit_pct);
                        }
                        sleep_thread(Duration::from_millis(interval_ms));
                    }
                })
                .expect("failed to spawn memory collector thread");
            handles.push(("memory", h));
        }

        // Channels for thread expansion requests.
        let (thread_req_tx, thread_req_rx) = mpsc::channel::<u32>();
        let (thread_res_tx, thread_res_rx) = mpsc::channel::<(u32, Vec<ThreadEntry>)>();

        {
            let state = Arc::clone(&proc_state);
            // Clone so the process thread can re-queue refresh requests for
            // already-expanded processes each collection cycle.
            let auto_thread_req_tx = thread_req_tx.clone();
            let h = ThreadBuilder::new()
                .name("wtop-process".into())
                .spawn(move || {
                    let mut collector =
                        process::ProcessCollector::new(thread_req_rx, thread_res_tx);
                    loop {
                        let mut snapshot = collector.collect();

                        // Drain expanded/thread state via a write lock so we can move
                        // the thread Vecs out (mem::take) instead of cloning them.
                        let mut preserve: HashMap<u32, (bool, Vec<ThreadEntry>)> = {
                            if let Ok(mut s) = state.write() {
                                s.iter_mut()
                                    .filter(|p| p.expanded || !p.threads.is_empty())
                                    .map(|p| (p.pid, (p.expanded, std::mem::take(&mut p.threads))))
                                    .collect()
                            } else {
                                HashMap::new()
                            }
                        };

                        // Merge preserved state into the new snapshot.
                        for entry in snapshot.iter_mut() {
                            if let Some((expanded, threads)) = preserve.remove(&entry.pid) {
                                entry.expanded = expanded;
                                if !threads.is_empty() {
                                    entry.threads = threads; // move, not clone
                                }
                            }
                        }

                        // Write lock only for the swap - minimal critical section.
                        if let Ok(mut s) = state.write() {
                            *s = snapshot;
                        }

                        // Re-queue thread refresh requests for all currently-expanded
                        // processes. These are drained by collector.collect() next cycle,
                        // giving live cpu_pct deltas on every process collection interval.
                        if let Ok(s) = state.read() {
                            for p in s.iter().filter(|p| p.expanded) {
                                let _ = auto_thread_req_tx.send(p.pid);
                            }
                        }

                        sleep_thread(Duration::from_millis(interval_ms * 2));
                    }
                })
                .expect("failed to spawn process collector thread");
            handles.push(("process", h));
        }

        {
            let state = Arc::clone(&disk_state);
            let h = ThreadBuilder::new()
                .name("wtop-disk".into())
                .spawn(move || {
                    let mut collector = disk::DiskCollector::new();
                    loop {
                        let mut snapshot = collector.collect();
                        if let Ok(mut s) = state.write() {
                            for d in &mut snapshot {
                                if let Some(existing) = s.iter().find(|e| e.drive == d.drive) {
                                    d.util_history = existing.util_history.clone();
                                } else {
                                    d.util_history = crate::models::RingBuffer::new(history_len);
                                }
                                d.util_history.push(d.utilization_pct);
                            }
                            *s = snapshot;
                        }
                        sleep_thread(Duration::from_millis(interval_ms));
                    }
                })
                .expect("failed to spawn disk collector thread");
            handles.push(("disk", h));
        }

        {
            let state = Arc::clone(&net_state);
            let h = ThreadBuilder::new()
                .name("wtop-network".into())
                .spawn(move || {
                    let mut collector = network::NetCollector::new();
                    loop {
                        let snapshot = collector.collect();
                        if let Ok(mut s) = state.write() {
                            *s = snapshot;
                        }
                        sleep_thread(Duration::from_millis(interval_ms));
                    }
                })
                .expect("failed to spawn network collector thread");
            handles.push(("network", h));
        }

        {
            let state = Arc::clone(&svc_state);
            let h = ThreadBuilder::new()
                .name("wtop-services".into())
                .spawn(move || {
                    let collector = services::ServicesCollector::new();
                    loop {
                        let snapshot = collector.collect();
                        if let Ok(mut s) = state.write() {
                            *s = snapshot;
                        }
                        sleep_thread(Duration::from_millis(5000));
                    }
                })
                .expect("failed to spawn services collector thread");
            handles.push(("services", h));
        }

        {
            let state = Arc::clone(&gpu_state);
            let h = ThreadBuilder::new()
                .name("wtop-gpu".into())
                .spawn(move || {
                    let mut collector = gpu::GpuCollector::new(history_len);
                    loop {
                        let snapshot = collector.collect(history_len);
                        if let Ok(mut s) = state.write() {
                            // Preserve per-GPU history across snapshots.
                            for (i, adapter) in snapshot.iter().enumerate() {
                                if let Some(existing) = s.get_mut(i) {
                                    existing.vram_used_bytes = adapter.vram_used_bytes;
                                    existing.utilization_pct = adapter.utilization_pct;
                                    existing.util_history.push(adapter.utilization_pct);
                                } else {
                                    s.push(adapter.clone());
                                    if let Some(a) = s.last_mut() {
                                        a.util_history.push(a.utilization_pct);
                                    }
                                }
                            }
                        }
                        sleep_thread(Duration::from_millis(interval_ms));
                    }
                })
                .expect("failed to spawn gpu collector thread");
            handles.push(("gpu", h));
        }

        Self {
            cpu: cpu_state,
            memory: mem_state,
            processes: proc_state,
            disks: disk_state,
            networks: net_state,
            gpus: gpu_state,
            services: svc_state,
            thread_request_tx: thread_req_tx,
            thread_result_rx: thread_res_rx,
            collector_handles: handles,
        }
    }
}
