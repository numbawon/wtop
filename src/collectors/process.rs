use bytesize::ByteSize;
use crate::models::process::{ProcessEntry, ProcessStatus};
use crate::models::thread::ThreadEntry;
use sysinfo::{ProcessStatus as SysProcessStatus, System};
use std::collections::HashMap;
use std::sync::{mpsc, Arc};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, LookupAccountSidW, SID_NAME_USE,
    TokenUser, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, THREADENTRY32, TH32CS_SNAPTHREAD,
};
use windows::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_QUERY_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::core::PWSTR;

pub struct ProcessCollector {
    sys: System,
    pub thread_request_rx: mpsc::Receiver<u32>,
    pub thread_result_tx: mpsc::Sender<(u32, Vec<ThreadEntry>)>,
    /// PID → interned username. Evicted when the process exits.
    user_cache: HashMap<u32, Arc<str>>,
    /// Unique username strings shared across all processes with the same owner.
    user_intern: HashMap<String, Arc<str>>,
    /// PID → (Instant of last collection, TID → (kernel_ms, user_ms)).
    /// Used to compute per-thread CPU% as a delta rate.
    thread_prev_times: HashMap<u32, (std::time::Instant, HashMap<u32, (u64, u64)>)>,
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
            user_cache: HashMap::new(),
            user_intern: HashMap::new(),
            thread_prev_times: HashMap::new(),
        }
    }

    pub fn collect(&mut self) -> Vec<ProcessEntry> {
        self.sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let total_memory = self.sys.total_memory().max(1);
        let thread_counts = count_threads_by_pid();

        // Resolve user arcs before building entries (can't borrow both cache maps
        // inside a single closure that also holds &mut self).
        let pid_users: HashMap<u32, Arc<str>> = self
            .sys
            .processes()
            .keys()
            .map(|pid_obj| {
                let pid = pid_obj.as_u32();
                let arc = if let Some(a) = self.user_cache.get(&pid) {
                    a.clone()
                } else {
                    let raw = resolve_process_user(pid);
                    let a = match self.user_intern.get(&raw) {
                        Some(existing) => existing.clone(),
                        None => {
                            let a: Arc<str> = Arc::from(raw.as_str());
                            self.user_intern.insert(raw, a.clone());
                            a
                        }
                    };
                    self.user_cache.insert(pid, a.clone());
                    a
                };
                (pid, arc)
            })
            .collect();

        let entries: Vec<ProcessEntry> = self
            .sys
            .processes()
            .values()
            .map(|p| {
                let pid = p.pid().as_u32();
                let mem = p.memory();
                let cpu_pct = p.cpu_usage();
                let mem_pct_display = mem as f32 / total_memory as f32 * 100.0;
                let thread_count = *thread_counts.get(&pid).unwrap_or(&0);
                let disk_usage = p.disk_usage();
                let disk_read = disk_usage.read_bytes;
                let disk_write = disk_usage.written_bytes;
                let user = pid_users.get(&pid).cloned().unwrap_or_else(|| Arc::from("?"));
                ProcessEntry {
                    pid,
                    pid_str: pid.to_string(),
                    name: p.name().to_string_lossy().into_owned(),
                    cpu_pct,
                    cpu_pct_str: format!(" {:>5.1}%", cpu_pct),
                    mem_bytes: mem,
                    mem_str: ByteSize(mem).to_string(),
                    mem_pct_str: format!("{:>4.1}%", mem_pct_display),
                    user,
                    status: map_status(p.status()),
                    thread_count,
                    thread_count_str: thread_count.to_string(),
                    disk_read_bps: disk_read,
                    disk_write_bps: disk_write,
                    disk_read_str: format!("{}/s", ByteSize(disk_read)),
                    disk_write_str: format!("{}/s", ByteSize(disk_write)),
                    expanded: false,
                    threads: Vec::new(),
                }
            })
            .collect();

        // Evict cache entries for PIDs that no longer exist.
        let live_pids: std::collections::HashSet<u32> = entries.iter().map(|e| e.pid).collect();
        self.user_cache.retain(|pid, _| live_pids.contains(pid));

        // Handle any pending thread expansion / refresh requests.
        while let Ok(pid) = self.thread_request_rx.try_recv() {
            let mut threads = crate::collectors::thread::collect_threads(pid);
            let now = std::time::Instant::now();

            // Compute live CPU% as a delta against the previous collection.
            if let Some((prev_instant, prev_map)) = self.thread_prev_times.get(&pid) {
                let elapsed_ms = prev_instant.elapsed().as_millis() as f64;
                if elapsed_ms > 0.0 {
                    for t in &mut threads {
                        if let Some(&(prev_k, prev_u)) = prev_map.get(&t.tid) {
                            let total_now = t.kernel_ms + t.user_ms;
                            let total_prev = prev_k + prev_u;
                            let delta = total_now.saturating_sub(total_prev) as f64;
                            t.cpu_pct = (delta / elapsed_ms * 100.0) as f32;
                        }
                    }
                }
            }

            // Update prev-times snapshot for this PID.
            let tid_map: HashMap<u32, (u64, u64)> = threads
                .iter()
                .map(|t| (t.tid, (t.kernel_ms, t.user_ms)))
                .collect();
            self.thread_prev_times.insert(pid, (now, tid_map));

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

/// Resolve the owner username for a process via OpenProcessToken + LookupAccountSidW.
/// Falls back to "?" for kernel processes or those that deny access.
fn resolve_process_user(pid: u32) -> String {
    // PID 0 (Idle) and PID 4 (System) are kernel entities with no accessible token.
    if pid == 0 || pid == 4 {
        return "SYSTEM".into();
    }
    unsafe { win32_process_user(pid) }.unwrap_or_else(|| "?".into())
}

unsafe fn win32_process_user(pid: u32) -> Option<String> {
    // Try full query rights first; fall back to limited (works on more processes on Win10/11).
    let proc = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid)
        .or_else(|_| OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid))
        .ok()?;

    let mut token = HANDLE::default();
    if OpenProcessToken(proc, TOKEN_QUERY, &mut token).is_err() {
        let _ = CloseHandle(proc);
        return None;
    }
    let _ = CloseHandle(proc);

    // First call: get required buffer size.
    let mut needed: u32 = 0;
    let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
    if needed == 0 {
        let _ = CloseHandle(token);
        return None;
    }

    let mut buf: Vec<u8> = vec![0u8; needed as usize];
    if GetTokenInformation(
        token,
        TokenUser,
        Some(buf.as_mut_ptr() as *mut _),
        needed,
        &mut needed,
    )
    .is_err()
    {
        let _ = CloseHandle(token);
        return None;
    }
    let _ = CloseHandle(token);

    // Extract the SID from TOKEN_USER.
    let token_user = &*(buf.as_ptr() as *const TOKEN_USER);
    let sid = token_user.User.Sid;

    // Resolve SID → account name.
    let mut name_buf = [0u16; 256];
    let mut domain_buf = [0u16; 256];
    let mut name_len = 256u32;
    let mut domain_len = 256u32;
    let mut sid_type = SID_NAME_USE(0);

    LookupAccountSidW(
        None,
        sid,
        PWSTR(name_buf.as_mut_ptr()),
        &mut name_len,
        PWSTR(domain_buf.as_mut_ptr()),
        &mut domain_len,
        &mut sid_type,
    )
    .ok()?;

    let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
    if name.is_empty() { None } else { Some(name) }
}

fn map_status(status: SysProcessStatus) -> ProcessStatus {
    match status {
        SysProcessStatus::Run => ProcessStatus::Running,
        SysProcessStatus::Stop => ProcessStatus::Suspended,
        SysProcessStatus::Zombie => ProcessStatus::Zombie,
        _ => ProcessStatus::Unknown,
    }
}
