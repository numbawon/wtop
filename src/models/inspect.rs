use crate::models::process::IntegrityLevel;
use crate::models::thread::ThreadEntry;

/// One loaded module (DLL / EXE segment) inside a process.
#[derive(Clone, Debug, Default)]
pub struct ModuleEntry {
    /// Filename only (e.g. "ntdll.dll").
    pub name: String,
    /// Full Win32 path.
    #[allow(dead_code)]
    pub path: String,
    /// Base load address.
    pub base: u64,
    /// Size of the image in bytes (SizeOfImage from MODULEINFO).
    pub size: u32,
}

/// One open kernel handle in a process.
#[derive(Clone, Debug)]
pub struct HandleEntry {
    /// Kernel handle value (used to force-close via DuplicateHandle).
    pub handle_value: u64,
    /// Object type name from NtQueryObject (e.g. "File", "Key", "Event").
    pub type_name: String,
    /// Resolved name or path; empty if not resolvable without risk of blocking.
    pub name: String,
}

/// One TCP or UDP network endpoint belonging to a process.
#[derive(Clone, Debug)]
pub struct NetworkEntry {
    /// Protocol family: "TCP4", "TCP6", "UDP4", "UDP6".
    pub proto: &'static str,
    /// Formatted "addr:port" for the local endpoint.
    pub local_addr: String,
    /// Formatted "addr:port" for the remote end; empty for UDP / LISTEN.
    pub remote_addr: String,
    /// TCP state string ("LISTEN", "ESTABLISHED", …); empty for UDP.
    pub state: &'static str,
}

/// Data collected on demand when the user presses `i` on a selected process.
#[derive(Clone, Debug, Default)]
pub struct ProcessInspectData {
    pub pid: u32,
    pub name: String,
    /// Full Win32 path to the executable (QueryFullProcessImageNameW).
    pub exe_path: String,
    /// Process command line (NtQueryInformationProcess class 60).
    pub cmdline: String,
    /// Seconds elapsed since the process was created.
    pub uptime_secs: u64,
    /// Absolute start time formatted as a local datetime string.
    pub start_time_str: String,
    /// Cumulative user-mode CPU time in milliseconds.
    pub cpu_user_ms: u64,
    /// Cumulative kernel-mode CPU time in milliseconds.
    pub cpu_kernel_ms: u64,
    /// Current working set size in bytes (GetProcessMemoryInfo).
    pub mem_working_set: u64,
    /// Peak working set size in bytes.
    pub mem_peak_ws: u64,
    /// Total page fault count.
    pub mem_page_faults: u32,
    /// True if this is a 32-bit process running under WoW64.
    pub arch_x86: bool,
    /// Priority class string (e.g. "Normal", "High", "Realtime").
    pub priority_class: String,
    /// FileVersion string from the PE version resource.
    pub file_version: Option<String>,
    /// ProductVersion string from the PE version resource.
    pub product_version: Option<String>,
    /// CompanyName string from the PE version resource.
    pub company_name: Option<String>,
    /// FileDescription string from the PE version resource.
    pub file_description: Option<String>,
    /// Parent process ID (from NtQueryInformationProcess/ProcessBasicInformation).
    pub parent_pid: u32,
    /// Parent process name (exe filename only).
    pub parent_name: String,
    /// Process integrity level from the process token.
    pub integrity: IntegrityLevel,
    /// Data Execution Prevention enabled (GetProcessMitigationPolicy).
    pub dep_enabled: Option<bool>,
    /// Address Space Layout Randomization enabled.
    pub aslr_enabled: Option<bool>,
    /// Control Flow Guard enabled.
    pub cfg_enabled: Option<bool>,
    /// Title of the process's main visible window, if any.
    pub window_title: Option<String>,
    /// All modules loaded into the process (EnumProcessModules), main exe first.
    pub modules: Vec<ModuleEntry>,
    /// All open kernel handles in the process (File, Key, Event, …).
    pub open_handles: Vec<HandleEntry>,
    /// TCP/UDP endpoints belonging to this process.
    pub open_connections: Vec<NetworkEntry>,
    /// Environment variables sorted by key.
    pub env_vars: Vec<(String, String)>,
    /// Threads collected at inspect time.
    pub threads: Vec<ThreadEntry>,
}

impl ProcessInspectData {
    pub fn uptime_display(&self) -> String {
        format_uptime(self.uptime_secs)
    }
}

pub fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}
