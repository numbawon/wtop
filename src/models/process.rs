use super::thread::ThreadEntry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Suspended,
    Zombie,
    Unknown,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessStatus::Running => write!(f, "Running"),
            ProcessStatus::Suspended => write!(f, "Suspend"),
            ProcessStatus::Zombie => write!(f, "Zombie"),
            ProcessStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f32,
    pub mem_bytes: u64,
    pub mem_pct: f32,
    pub user: String,
    pub status: ProcessStatus,
    pub thread_count: u32,
    pub disk_read_bps: u64,
    pub disk_write_bps: u64,
    /// Whether the thread list is expanded in the UI.
    pub expanded: bool,
    /// Populated on demand when user expands the row.
    pub threads: Vec<ThreadEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessSortField {
    Pid,
    Name,
    CpuPct,
    MemBytes,
    ThreadCount,
    DiskRead,
    DiskWrite,
}

impl ProcessSortField {
    /// Cycle to the next sort field.
    pub fn next(self) -> Self {
        match self {
            Self::Pid => Self::Name,
            Self::Name => Self::CpuPct,
            Self::CpuPct => Self::MemBytes,
            Self::MemBytes => Self::ThreadCount,
            Self::ThreadCount => Self::DiskRead,
            Self::DiskRead => Self::DiskWrite,
            Self::DiskWrite => Self::Pid,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Pid => Self::DiskWrite,
            Self::Name => Self::Pid,
            Self::CpuPct => Self::Name,
            Self::MemBytes => Self::CpuPct,
            Self::ThreadCount => Self::MemBytes,
            Self::DiskRead => Self::ThreadCount,
            Self::DiskWrite => Self::DiskRead,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pid => "PID",
            Self::Name => "NAME",
            Self::CpuPct => "CPU%",
            Self::MemBytes => "MEM",
            Self::ThreadCount => "THDS",
            Self::DiskRead => "DISK-R",
            Self::DiskWrite => "DISK-W",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SortState {
    pub field: ProcessSortField,
    pub ascending: bool,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            field: ProcessSortField::CpuPct,
            ascending: false,
        }
    }
}

pub fn sort_processes(processes: &mut Vec<ProcessEntry>, sort: SortState) {
    processes.sort_by(|a, b| {
        let ord = match sort.field {
            ProcessSortField::Pid => a.pid.cmp(&b.pid),
            ProcessSortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            ProcessSortField::CpuPct => a
                .cpu_pct
                .partial_cmp(&b.cpu_pct)
                .unwrap_or(std::cmp::Ordering::Equal),
            ProcessSortField::MemBytes => a.mem_bytes.cmp(&b.mem_bytes),
            ProcessSortField::ThreadCount => a.thread_count.cmp(&b.thread_count),
            ProcessSortField::DiskRead => a.disk_read_bps.cmp(&b.disk_read_bps),
            ProcessSortField::DiskWrite => a.disk_write_bps.cmp(&b.disk_write_bps),
        };
        if sort.ascending {
            ord
        } else {
            ord.reverse()
        }
    });
}
