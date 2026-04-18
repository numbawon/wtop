#[derive(Clone, Debug)]
pub enum ServiceStatus {
    Running,
    Stopped,
    StartPending,
    StopPending,
    ContinuePending,
    PausePending,
    Paused,
    Unknown,
}

impl ServiceStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running         => "Running",
            Self::Stopped         => "Stopped",
            Self::StartPending    => "Starting",
            Self::StopPending     => "Stopping",
            Self::ContinuePending => "Resuming",
            Self::PausePending    => "Pausing",
            Self::Paused          => "Paused",
            Self::Unknown         => "?",
        }
    }

}

#[derive(Clone, Debug)]
pub enum ServiceStartType {
    Boot,
    System,
    Auto,
    Manual,
    Disabled,
    Unknown,
}

impl ServiceStartType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Boot     => "Boot",
            Self::System   => "System",
            Self::Auto     => "Auto",
            Self::Manual   => "Manual",
            Self::Disabled => "Disabled",
            Self::Unknown  => "?",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServiceEntry {
    pub name: String,
    pub display_name: String,
    pub status: ServiceStatus,
    pub start_type: ServiceStartType,
    pub pid: u32,
}
