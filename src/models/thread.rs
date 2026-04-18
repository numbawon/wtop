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
    pub state: ThreadState,
    /// Kernel CPU time in milliseconds (from GetThreadTimes).
    pub kernel_ms: u64,
    /// User CPU time in milliseconds (from GetThreadTimes).
    pub user_ms: u64,
    /// Live CPU utilisation percent - delta of (kernel_ms+user_ms) over the last
    /// collection interval. 0.0 on first collection (no prior sample).
    pub cpu_pct: f32,
    pub priority: i32,
    /// Module name of the thread's start address (e.g. "ntdll.dll").
    pub start_module: String,
    /// Raw start address - shown in the User column for suspicious threads.
    pub start_address: u64,
    /// True if the start address resolves to a module NOT in the process module list.
    /// Heuristic indicator for potential DLL injection.
    pub suspicious: bool,
    /// Raw Windows wait reason code (only meaningful when state == Waiting).
    /// Use `wait_reason_label()` to get a human-readable short string.
    pub wait_reason: u32,
    /// Thread description string set by the thread itself via
    /// SetThreadDescription / NtSetInformationThread. Present on most
    /// .NET, Java, and Chromium threads; None for native-only threads.
    pub name: Option<String>,
}

/// Convert a Windows KWAIT_REASON value to a short display label.
pub fn wait_reason_label(reason: u32) -> &'static str {
    match reason {
        0 | 7  => "Exec",     // WrExecutive
        1 | 8  => "FreePg",   // WrFreePage
        2 | 9  => "PageIn",   // WrPageIn
        3 | 10 => "Pool",     // WrPoolAllocation
        4 | 11 => "Sleep",    // WrDelayExecution
        5 | 12 => "Suspnd",   // WrSuspended
        6 | 13 => "User",     // WrUserRequest (UI / message queue)
        14     => "EvtPair",  // WrEventPair
        15     => "Queue",    // WrQueue (thread pool worker)
        16     => "LpcRecv",  // WrLpcReceive
        17     => "LpcRply",  // WrLpcReply
        18     => "VirtMem",  // WrVirtualMemory
        19     => "PageOut",  // WrPageOut
        26     => "Kernel",   // WrKernel
        27     => "Rsrc",     // WrResource
        28     => "Lock",     // WrPushLock
        29     => "Mutex",    // WrMutex
        32     => "Prempt",   // WrPreempted
        35     => "GrdMtx",   // WrGuardedMutex
        _      => "Wait",
    }
}
