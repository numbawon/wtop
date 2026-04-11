use crate::models::process::{ProcessEntry, ProcessStatus};
use crate::models::thread::ThreadEntry;
use sysinfo::{ProcessStatus as SysProcessStatus, System};
use std::sync::mpsc;

pub struct ProcessCollector {
    sys: System,
    /// Receives PIDs from the UI thread requesting thread expansion.
    pub thread_request_rx: mpsc::Receiver<u32>,
    /// Sends populated thread lists back to the app state.
    pub thread_result_tx: mpsc::Sender<(u32, Vec<ThreadEntry>)>,
}

impl ProcessCollector {
    pub fn new(
        thread_request_rx: mpsc::Receiver<u32>,
        thread_result_tx: mpsc::Sender<(u32, Vec<ThreadEntry>)>,
    ) -> Self {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        Self {
            sys,
            thread_request_rx,
            thread_result_tx,
        }
    }

    /// Collect the current process list and handle any pending thread requests.
    pub fn collect(&mut self) -> Vec<ProcessEntry> {
        self.sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let total_memory = self.sys.total_memory().max(1);

        let entries: Vec<ProcessEntry> = self
            .sys
            .processes()
            .values()
            .map(|p| {
                let mem = p.memory();
                ProcessEntry {
                    pid: p.pid().as_u32(),
                    name: p.name().to_string_lossy().into_owned(),
                    cpu_pct: p.cpu_usage(),
                    mem_bytes: mem,
                    mem_pct: mem as f32 / total_memory as f32 * 100.0,
                    user: p
                        .user_id()
                        .map(|u| u.to_string())
                        .unwrap_or_else(|| "?".into()),
                    status: map_status(p.status()),
                    thread_count: 0, // populated by thread collector
                    disk_read_bps: p.disk_usage().read_bytes,
                    disk_write_bps: p.disk_usage().written_bytes,
                    expanded: false,
                    threads: Vec::new(),
                }
            })
            .collect();

        // Handle any pending thread expansion requests.
        while let Ok(pid) = self.thread_request_rx.try_recv() {
            let threads = crate::collectors::thread::collect_threads(pid);
            let _ = self.thread_result_tx.send((pid, threads));
        }

        entries
    }
}

fn map_status(status: SysProcessStatus) -> ProcessStatus {
    match status {
        SysProcessStatus::Run => ProcessStatus::Running,
        SysProcessStatus::Stop => ProcessStatus::Suspended,
        SysProcessStatus::Zombie => ProcessStatus::Zombie,
        _ => ProcessStatus::Unknown,
    }
}
