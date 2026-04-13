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
    /// FileVersion string from the PE version resource.
    pub file_version: Option<String>,
    /// ProductVersion string from the PE version resource.
    pub product_version: Option<String>,
    /// CompanyName string from the PE version resource.
    pub company_name: Option<String>,
    /// FileDescription string from the PE version resource.
    pub file_description: Option<String>,
    /// All modules loaded into the process (EnumProcessModules), main exe first.
    pub modules: Vec<ModuleEntry>,
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
