mod app;
mod collectors;
mod config;
mod glyphs;
mod input;
mod models;
mod ui;
mod wt;

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

    /// Enable Nerd Font glyphs for panel icons (requires a Nerd Font in your terminal)
    #[arg(long)]
    nerd_glyphs: bool,

    /// Disable Nerd Font glyphs (overrides --nerd-glyphs)
    #[arg(long)]
    no_nerd_glyphs: bool,

    /// Force ASCII-only borders and sparklines (for minimal/legacy terminals)
    #[arg(long)]
    ascii: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize file-based logging so stdout stays clean for the TUI.
    init_logging(&args.log_level)?;

    let nerd_glyphs = if args.no_nerd_glyphs {
        false
    } else if args.nerd_glyphs {
        true
    } else {
        // Auto-hint: if running inside Windows Terminal, default to enabled so
        // users who have already applied a Nerd Font get icons immediately.
        std::env::var("WT_SESSION").is_ok()
    };

    let config = Config {
        refresh_interval_ms: args.interval.clamp(250, 5000),
        theme: match args.theme.to_lowercase().as_str() {
            "light"            => ThemeName::Light,
            "dracula"          => ThemeName::Dracula,
            "gruvbox"          => ThemeName::Gruvbox,
            "catppuccin"       => ThemeName::CatppuccinMocha,
            "catppuccin_mocha" => ThemeName::CatppuccinMocha,
            "nord"             => ThemeName::Nord,
            "tokyo_night" | "tokyonight" => ThemeName::TokyoNight,
            _                  => ThemeName::Dark,
        },
        nerd_glyphs,
        ascii_mode: args.ascii || std::env::var("NO_COLOR").is_ok(),
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
