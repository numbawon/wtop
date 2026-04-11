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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeName {
    Dark,
    Light,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 1000,
            cpu_history_len: 60,
            theme: ThemeName::Dark,
            show_system_processes: true,
        }
    }
}
