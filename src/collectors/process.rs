use crate::models::process::{ProcessEntry, ProcessStatus};
use crate::models::thread::ThreadEntry;
use sysinfo::{ProcessStatus as SysProcessStatus, System};
use std::collections::HashMap;
use std::sync::mpsc;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, THREADENTRY32, TH32CS_SNAPTHREAD,
};

pub struct ProcessCollector {
    sys: System,
    pub thread_request_rx: mpsc::Receiver<u32>,
    pub thread_result_tx: mpsc::Sender<(u32, Vec<ThreadEntry>)>,
}

impl ProcessCollector {
    pub fn new(
        thread_request_rx: mpsc::Receiver<u32>,
        thread_result_tx: mpsc::Sender<(u32, Vec<ThreadEntry>)>,
    ) -> Self {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        Self { sys, thread_request_rx, thread_result_tx }
    }

    pub fn collect(&mut self) -> Vec<ProcessEntry> {
        self.sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let total_memory = self.sys.total_memory().max(1);
        let thread_counts = count_threads_by_pid();

        let entries: Vec<ProcessEntry> = self
            .sys
            .processes()
            .values()
            .map(|p| {
                let pid = p.pid().as_u32();
                let mem = p.memory();
                ProcessEntry {
                    pid,
                    name: p.name().to_string_lossy().into_owned(),
                    cpu_pct: p.cpu_usage(),
                    mem_bytes: mem,
                    mem_pct: mem as f32 / total_memory as f32 * 100.0,
                    user: p
                        .user_id()
                        .map(|u| u.to_string())
                        .unwrap_or_else(|| "?".into()),
                    status: map_status(p.status()),
                    thread_count: *thread_counts.get(&pid).unwrap_or(&0),
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

/// One Toolhelp32 snapshot pass to count threads per PID.
fn count_threads_by_pid() -> HashMap<u32, u32> {
    let mut counts: HashMap<u32, u32> = HashMap::new();

    let snapshot = unsafe {
        match CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            Ok(h) => h,
            Err(_) => return counts,
        }
    };

    let mut te = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };

    if unsafe { Thread32First(snapshot, &mut te) }.is_ok() {
        loop {
            *counts.entry(te.th32OwnerProcessID).or_insert(0) += 1;
            te.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            if unsafe { Thread32Next(snapshot, &mut te) }.is_err() {
                break;
            }
        }
    }

    unsafe { let _ = CloseHandle(snapshot); }
    counts
}

fn map_status(status: SysProcessStatus) -> ProcessStatus {
    match status {
        SysProcessStatus::Run => ProcessStatus::Running,
        SysProcessStatus::Stop => ProcessStatus::Suspended,
        SysProcessStatus::Zombie => ProcessStatus::Zombie,
        _ => ProcessStatus::Unknown,
    }
}
