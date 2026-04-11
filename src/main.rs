mod app;
mod collectors;
mod config;
mod error;
mod input;
mod models;
mod ui;

use clap::Parser;
use config::{Config, ThemeName};
use tracing_subscriber::EnvFilter;

/// wtop — Windows terminal system monitor
#[derive(Parser, Debug)]
#[command(name = "wtop", about = "htop-style system monitor for Windows")]
struct Args {
    /// Refresh interval in milliseconds (250–5000)
    #[arg(short, long, default_value_t = 1000)]
    interval: u64,

    /// Color theme: dark or light
    #[arg(short, long, default_value = "dark")]
    theme: String,

    /// Log verbosity (off, error, warn, info, debug, trace)
    #[arg(long, default_value = "warn")]
    log_level: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize file-based logging so stdout stays clean for the TUI.
    init_logging(&args.log_level)?;

    let config = Config {
        refresh_interval_ms: args.interval.clamp(250, 5000),
        theme: if args.theme == "light" { ThemeName::Light } else { ThemeName::Dark },
        ..Config::default()
    };

    app::run(config)
}

fn init_logging(level: &str) -> anyhow::Result<()> {
    let log_dir = std::env::temp_dir();
    let file_appender = tracing_appender::rolling::never(&log_dir, "wtop.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Intentionally leak _guard so the logger stays alive for the process lifetime.
    std::mem::forget(_guard);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(level))
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    Ok(())
}
