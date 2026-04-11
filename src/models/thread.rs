#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadState {
    Running,
    Waiting,
    Suspended,
    Terminated,
    Unknown,
}

impl std::fmt::Display for ThreadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadState::Running => write!(f, "Running"),
            ThreadState::Waiting => write!(f, "Waiting"),
            ThreadState::Suspended => write!(f, "Suspend"),
            ThreadState::Terminated => write!(f, "Terminat"),
            ThreadState::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThreadEntry {
    pub tid: u32,
    pub owner_pid: u32,
    pub state: ThreadState,
    /// Kernel + user CPU time combined in milliseconds.
    pub cpu_time_ms: u64,
    pub priority: i32,
    /// Module name of the thread's start address (e.g. "ntdll.dll").
    pub start_module: String,
    /// Raw start address — shown for flagged threads.
    pub start_address: u64,
    /// True if the start address resolves to a module NOT in the process module list.
    /// Heuristic indicator for potential DLL injection.
    pub suspicious: bool,
}
