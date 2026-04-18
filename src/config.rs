use serde::{Deserialize, Serialize};

/// Identifies a single column in the process table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum LayoutMode {
    /// Auto-select based on terminal width (default).
    #[default]
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

    pub fn cycle_back(&self) -> Self {
        match self {
            Self::Auto    => Self::Stacked,
            Self::Compact => Self::Auto,
            Self::Wide    => Self::Compact,
            Self::Stacked => Self::Wide,
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GaugeStyle {
    /// Filled solid block - standard ratatui `Gauge` widget.
    Block,
    /// Thin horizontal line - ratatui `LineGauge` widget.
    Line,
    /// Sub-cell Unicode block elements (▏▎▍▌▋▊▉█) for a smoother bar.
    Segmented,
    /// Plain ASCII `[===   ]` bracket bar, works in any terminal.
    Ascii,
}

/// Application configuration - persisted to %APPDATA%\wtop\config.toml.
///
/// `#[serde(default)]` ensures that fields added in future versions will
/// deserialize to their `Default` value rather than failing on old config files.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// How often collectors refresh data, in milliseconds.
    pub refresh_interval_ms: u64,
    /// Number of CPU history samples to keep for sparklines.
    pub cpu_history_len: usize,
    /// Active color theme slug - matches a filename in the themes directory
    /// ("dark", "dracula", "my_theme", …).
    pub theme: String,
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
    #[serde(default = "default_process_columns")]
    pub process_columns: Vec<ProcessColumn>,
    /// Force ASCII borders and sparkline chars regardless of NO_COLOR.
    pub ascii_mode: bool,
    /// Hide all auto-detected virtual adapters (Hyper-V, Docker, VMware, WSL, …).
    pub hide_virtual_adapters: bool,
    /// Adapter display_names that are explicitly hidden in the network panel.
    #[serde(default)]
    pub hidden_adapters: Vec<String>,
    /// Use 24-hour clock in the status bar (false = 12-hour AM/PM).
    pub time_24h: bool,
    /// Show processes in parent/child tree view instead of flat list.
    pub tree_view: bool,
}

/// Normalise a theme name from any format into a lowercase slug.
/// Handles legacy PascalCase names serialised by the old `ThemeName` enum.
fn normalize_theme_slug(name: &str) -> String {
    match name.to_lowercase().replace(['-', ' '], "_").as_str() {
        "catppuccinmocha" | "catppuccin" => "catppuccin_mocha".to_string(),
        "tokyonight"                     => "tokyo_night".to_string(),
        other                            => other.to_string(),
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 1000,
            cpu_history_len: 60,
            theme: "dark".to_string(),
            show_system_processes: true,
            nerd_glyphs: false,
            layout_mode: LayoutMode::Auto,
            show_disk: true,
            show_network: true,
            process_columns: default_process_columns(),
            ascii_mode: false,
            hide_virtual_adapters: false,
            hidden_adapters: Vec::new(),
            time_24h: true,
            tree_view: false,
        }
    }
}

impl Config {
    fn config_path() -> std::path::PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
        std::path::PathBuf::from(appdata).join("wtop").join("config.toml")
    }

    /// Load config from disk, falling back to defaults on any error.
    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(mut config) = toml::from_str::<Config>(&text) {
                config.theme = normalize_theme_slug(&config.theme);
                return config;
            }
        }
        Self::default()
    }

    /// Persist config to %APPDATA%\wtop\config.toml.
    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create config directory: {e}");
                return;
            }
        }
        match toml::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    tracing::warn!("Failed to save config to {}: {e}", path.display());
                }
            }
            Err(e) => tracing::warn!("Failed to serialize config: {e}"),
        }
    }
}
