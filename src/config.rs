/// Identifies a single column in the process table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessColumnId {
    Pid,
    Name,
    CpuPct,
    Mem,
    MemPct,
    Threads,
    Status,
    User,
    DiskRead,
    DiskWrite,
}

/// A column in the process table with its current visibility.
#[derive(Clone, Debug)]
pub struct ProcessColumn {
    pub id: ProcessColumnId,
    pub visible: bool,
}

impl ProcessColumn {
    pub fn new(id: ProcessColumnId, visible: bool) -> Self {
        Self { id, visible }
    }
}

/// Default ordered column list. DiskRead/DiskWrite are hidden by default.
pub fn default_process_columns() -> Vec<ProcessColumn> {
    vec![
        ProcessColumn::new(ProcessColumnId::Pid,       true),
        ProcessColumn::new(ProcessColumnId::Name,      true),
        ProcessColumn::new(ProcessColumnId::CpuPct,    true),
        ProcessColumn::new(ProcessColumnId::Mem,       true),
        ProcessColumn::new(ProcessColumnId::MemPct,    true),
        ProcessColumn::new(ProcessColumnId::Threads,   true),
        ProcessColumn::new(ProcessColumnId::Status,    true),
        ProcessColumn::new(ProcessColumnId::User,      true),
        ProcessColumn::new(ProcessColumnId::DiskRead,  false),
        ProcessColumn::new(ProcessColumnId::DiskWrite, false),
    ]
}

/// How the panels are arranged on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    /// Auto-select based on terminal width (default).
    Auto,
    /// Force the compact/narrow stacked layout.
    Compact,
    /// Force the wide side-by-side layout regardless of terminal width.
    Wide,
    /// Everything in a single column; process list gets the most space.
    Stacked,
}

impl LayoutMode {
    pub fn cycle(&self) -> Self {
        match self {
            Self::Auto    => Self::Compact,
            Self::Compact => Self::Wide,
            Self::Wide    => Self::Stacked,
            Self::Stacked => Self::Auto,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto    => "Auto",
            Self::Compact => "Compact",
            Self::Wide    => "Wide",
            Self::Stacked => "Stacked",
        }
    }
}

/// How gauges and bars are drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GaugeStyle {
    /// Filled solid block — standard ratatui `Gauge` widget.
    Block,
    /// Thin horizontal line — ratatui `LineGauge` widget.
    Line,
    /// Sub-cell Unicode block elements (▏▎▍▌▋▊▉█) for a smoother bar.
    Segmented,
    /// Plain ASCII `[===   ]` bracket bar, works in any terminal.
    Ascii,
}

/// Application configuration — defaults used on first run.
/// Phase 3 will add reading from %APPDATA%\wtop\config.toml.
#[derive(Clone, Debug)]
pub struct Config {
    /// How often collectors refresh data, in milliseconds.
    pub refresh_interval_ms: u64,
    /// Number of CPU history samples to keep for sparklines.
    pub cpu_history_len: usize,
    /// Active color theme name.
    pub theme: ThemeName,
    /// Whether system processes are shown by default.
    pub show_system_processes: bool,
    /// Use Nerd Font private-use glyphs for panel icons and markers.
    pub nerd_glyphs: bool,
    /// Panel layout mode.
    pub layout_mode: LayoutMode,
    /// Whether the Disk panel is visible.
    pub show_disk: bool,
    /// Whether the Network panel is visible.
    pub show_network: bool,
    /// Ordered, per-column visibility for the process table.
    pub process_columns: Vec<ProcessColumn>,
    /// Force ASCII borders and sparkline chars regardless of NO_COLOR.
    pub ascii_mode: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeName {
    Dark,
    Light,
    Dracula,
    Gruvbox,
    CatppuccinMocha,
    Nord,
    TokyoNight,
}

impl ThemeName {
    /// Advance to the next theme in the cycle.
    pub fn cycle(&self) -> Self {
        match self {
            Self::Dark           => Self::Light,
            Self::Light          => Self::Dracula,
            Self::Dracula        => Self::Gruvbox,
            Self::Gruvbox        => Self::CatppuccinMocha,
            Self::CatppuccinMocha => Self::Nord,
            Self::Nord           => Self::TokyoNight,
            Self::TokyoNight     => Self::Dark,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Dark            => "Dark",
            Self::Light           => "Light",
            Self::Dracula         => "Dracula",
            Self::Gruvbox         => "Gruvbox",
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::Nord            => "Nord",
            Self::TokyoNight      => "Tokyo Night",
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 1000,
            cpu_history_len: 60,
            theme: ThemeName::Dark,
            show_system_processes: true,
            nerd_glyphs: false,
            layout_mode: LayoutMode::Auto,
            show_disk: true,
            show_network: true,
            process_columns: default_process_columns(),
            ascii_mode: false,
        }
    }
}
